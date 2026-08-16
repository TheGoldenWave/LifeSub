use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::raw::{c_char, c_int, c_void};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::ptr;
use std::sync::{Arc, Mutex, OnceLock};

use rusqlite::ffi;

const VFS_NAME: &CStr = c"lifesub-anchored";
const SYNTHETIC_ROOT: &str = "/lifesub-anchored";

pub(crate) struct AnchoredVfs {
    token: String,
    state: &'static GlobalState,
    database_path: String,
}

struct GlobalRegistration {
    state: &'static GlobalState,
}

struct GlobalState {
    native: usize,
    directories: Mutex<HashMap<String, Arc<DirectoryState>>>,
}

struct DirectoryState {
    directory: File,
    database_name: CString,
}

struct OpenIdentity {
    descriptor: File,
    _descriptor_path: CString,
    directory: File,
    entry: CString,
    original_methods: usize,
    patched_methods: usize,
}

static REGISTRATION: OnceLock<Result<GlobalRegistration, c_int>> = OnceLock::new();
static OPEN_IDENTITIES: OnceLock<Mutex<HashMap<usize, OpenIdentity>>> = OnceLock::new();
#[cfg(test)]
thread_local! {
    static FAIL_AFTER_NATIVE_OPEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAILED_OPEN_NATIVE_CLOSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

unsafe impl Send for GlobalRegistration {}
unsafe impl Sync for GlobalRegistration {}
unsafe impl Send for GlobalState {}
unsafe impl Sync for GlobalState {}

impl AnchoredVfs {
    pub(crate) fn register(directory: &File, database_name: &str) -> rusqlite::Result<Self> {
        let registration = registration()?;
        let token = uuid::Uuid::new_v4().simple().to_string();
        let directory = Arc::new(DirectoryState {
            directory: directory
                .try_clone()
                .map_err(to_rusqlite_conversion_error)?,
            database_name: CString::new(database_name).map_err(to_rusqlite_conversion_error)?,
        });
        registration
            .state
            .directories
            .lock()
            .map_err(|_| sqlite_error(ffi::SQLITE_IOERR, "anchored VFS registry is poisoned"))?
            .insert(token.clone(), directory);
        Ok(Self {
            database_path: format!("{token}/{database_name}"),
            token,
            state: registration.state,
        })
    }

    pub(crate) const fn name(&self) -> &CStr {
        VFS_NAME
    }

    pub(crate) fn database_path(&self) -> &str {
        &self.database_path
    }
}

impl Drop for AnchoredVfs {
    fn drop(&mut self) {
        if let Ok(mut directories) = self.state.directories.lock() {
            directories.remove(&self.token);
        }
    }
}

fn registration() -> rusqlite::Result<&'static GlobalRegistration> {
    REGISTRATION
        .get_or_init(|| unsafe { register_global_vfs() })
        .as_ref()
        .map_err(|code| sqlite_error(*code, "failed to register anchored SQLite VFS"))
}

unsafe fn register_global_vfs() -> Result<GlobalRegistration, c_int> {
    #[cfg(target_os = "macos")]
    let native = unsafe { ffi::sqlite3_vfs_find(c"unix-excl".as_ptr()) };
    #[cfg(not(target_os = "macos"))]
    let native = unsafe { ffi::sqlite3_vfs_find(ptr::null()) };
    if native.is_null() {
        return Err(ffi::SQLITE_NOTFOUND);
    }
    let state = Box::leak(Box::new(GlobalState {
        native: native as usize,
        directories: Mutex::new(HashMap::new()),
    }));
    let mut vfs = unsafe { *native };
    vfs.pNext = ptr::null_mut();
    vfs.zName = VFS_NAME.as_ptr();
    vfs.pAppData = (state as *mut GlobalState).cast::<c_void>();
    vfs.xOpen = Some(anchored_open);
    vfs.xDelete = Some(anchored_delete);
    vfs.xAccess = Some(anchored_access);
    vfs.xFullPathname = Some(anchored_full_pathname);
    let vfs = Box::leak(Box::new(vfs));
    let result = unsafe { ffi::sqlite3_vfs_register(vfs, 0) };
    if result != ffi::SQLITE_OK {
        return Err(result);
    }
    Ok(GlobalRegistration { state })
}

unsafe extern "C" fn anchored_open(
    vfs: *mut ffi::sqlite3_vfs,
    name: ffi::sqlite3_filename,
    file: *mut ffi::sqlite3_file,
    flags: c_int,
    output_flags: *mut c_int,
) -> c_int {
    let global = unsafe { global_state(vfs) };
    let native = global.native as *mut ffi::sqlite3_vfs;
    let Some(native_open) = (unsafe { (*native).xOpen }) else {
        return ffi::SQLITE_CANTOPEN;
    };
    if name.is_null() {
        return ffi::SQLITE_CANTOPEN;
    }
    let Some((directory, entry)) = (unsafe { anchored_entry(global, name) }) else {
        return ffi::SQLITE_CANTOPEN;
    };
    let (descriptor_file, created) =
        match open_entry(directory.directory.as_raw_fd(), &entry, flags) {
            Ok(descriptor) => descriptor,
            Err(code) => return code,
        };
    let descriptor_path = match CString::new(format!("/dev/fd/{}", descriptor_file.as_raw_fd())) {
        Ok(path) => path,
        Err(_) => return ffi::SQLITE_CANTOPEN,
    };
    let directory_file = match directory.directory.try_clone() {
        Ok(directory_file) => directory_file,
        Err(_) => return ffi::SQLITE_CANTOPEN,
    };
    let result =
        unsafe { native_open(native, descriptor_path.as_ptr(), file, flags, output_flags) };
    if result != ffi::SQLITE_OK {
        return result;
    }
    let original_methods = unsafe { (*file).pMethods };
    if original_methods.is_null() {
        return ffi::SQLITE_CANTOPEN;
    }
    #[cfg(test)]
    if FAIL_AFTER_NATIVE_OPEN.replace(false) {
        unsafe { close_native(file, original_methods) };
        return ffi::SQLITE_CANTOPEN;
    }
    if created && directory.directory.sync_all().is_err() {
        unsafe { close_native(file, original_methods) };
        return ffi::SQLITE_IOERR_DIR_FSYNC;
    }
    let mut patched_methods = unsafe { *original_methods };
    patched_methods.xClose = Some(anchored_close);
    patched_methods.xFileControl = Some(anchored_file_control);
    let patched_methods = Box::into_raw(Box::new(patched_methods));
    let mut identities = identities()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if identities.contains_key(&(file as usize)) {
        unsafe { close_native(file, original_methods) };
        unsafe { drop(Box::from_raw(patched_methods)) };
        return ffi::SQLITE_IOERR;
    }
    identities.insert(
        file as usize,
        OpenIdentity {
            descriptor: descriptor_file,
            _descriptor_path: descriptor_path,
            directory: directory_file,
            entry,
            original_methods: original_methods as usize,
            patched_methods: patched_methods as usize,
        },
    );
    unsafe {
        (*file).pMethods = patched_methods;
    }
    ffi::SQLITE_OK
}

unsafe fn close_native(file: *mut ffi::sqlite3_file, methods: *const ffi::sqlite3_io_methods) {
    unsafe {
        if let Some(close) = (*methods).xClose {
            let _ = close(file);
        }
        (*file).pMethods = ptr::null();
    }
    #[cfg(test)]
    FAILED_OPEN_NATIVE_CLOSES.set(FAILED_OPEN_NATIVE_CLOSES.get() + 1);
}

unsafe extern "C" fn anchored_close(file: *mut ffi::sqlite3_file) -> c_int {
    let mut identities = identities()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(identity) = identities.remove(&(file as usize)) else {
        return ffi::SQLITE_IOERR_CLOSE;
    };
    drop(identities);
    let original_methods = identity.original_methods as *const ffi::sqlite3_io_methods;
    unsafe {
        (*file).pMethods = original_methods;
    }
    let result = unsafe {
        (*original_methods)
            .xClose
            .map_or(ffi::SQLITE_IOERR_CLOSE, |close| close(file))
    };
    unsafe {
        drop(Box::from_raw(
            identity.patched_methods as *mut ffi::sqlite3_io_methods,
        ));
    }
    result
}

unsafe extern "C" fn anchored_file_control(
    file: *mut ffi::sqlite3_file,
    operation: c_int,
    argument: *mut c_void,
) -> c_int {
    let Ok(identities) = identities().lock() else {
        return ffi::SQLITE_IOERR;
    };
    let Some(identity) = identities.get(&(file as usize)) else {
        return ffi::SQLITE_IOERR;
    };
    if operation == ffi::SQLITE_FCNTL_HAS_MOVED {
        unsafe {
            *argument.cast::<c_int>() = entry_has_moved(identity) as c_int;
        }
        return ffi::SQLITE_OK;
    }
    let methods = identity.original_methods as *const ffi::sqlite3_io_methods;
    unsafe {
        (*methods)
            .xFileControl
            .map_or(ffi::SQLITE_NOTFOUND, |control| {
                control(file, operation, argument)
            })
    }
}

unsafe extern "C" fn anchored_delete(
    vfs: *mut ffi::sqlite3_vfs,
    name: *const c_char,
    sync_directory: c_int,
) -> c_int {
    let global = unsafe { global_state(vfs) };
    let Some((directory, entry)) = (unsafe { anchored_entry(global, name) }) else {
        return ffi::SQLITE_IOERR_DELETE;
    };
    match validate_entry_at(&directory.directory, &entry) {
        Ok(true) => {}
        Ok(false) => return ffi::SQLITE_OK,
        Err(_) => return ffi::SQLITE_IOERR_DELETE,
    }
    if unsafe { libc::unlinkat(directory.directory.as_raw_fd(), entry.as_ptr(), 0) } == -1 {
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::NotFound {
            return ffi::SQLITE_IOERR_DELETE;
        }
    }
    if sync_directory != 0 && directory.directory.sync_all().is_err() {
        return ffi::SQLITE_IOERR_DIR_FSYNC;
    }
    ffi::SQLITE_OK
}

unsafe extern "C" fn anchored_access(
    vfs: *mut ffi::sqlite3_vfs,
    name: *const c_char,
    flags: c_int,
    output: *mut c_int,
) -> c_int {
    let global = unsafe { global_state(vfs) };
    let Some((directory, entry)) = (unsafe { anchored_entry(global, name) }) else {
        return ffi::SQLITE_IOERR_ACCESS;
    };
    let valid = match validate_entry_at(&directory.directory, &entry) {
        Ok(valid) => valid,
        Err(_) => return ffi::SQLITE_IOERR_ACCESS,
    };
    unsafe {
        *output = if flags == ffi::SQLITE_ACCESS_EXISTS {
            valid as c_int
        } else {
            (valid
                && libc::faccessat(
                    directory.directory.as_raw_fd(),
                    entry.as_ptr(),
                    libc::R_OK,
                    0,
                ) == 0
                && (flags != ffi::SQLITE_ACCESS_READWRITE
                    || libc::faccessat(
                        directory.directory.as_raw_fd(),
                        entry.as_ptr(),
                        libc::W_OK,
                        0,
                    ) == 0)) as c_int
        };
    }
    ffi::SQLITE_OK
}

unsafe extern "C" fn anchored_full_pathname(
    vfs: *mut ffi::sqlite3_vfs,
    name: *const c_char,
    output_length: c_int,
    output: *mut c_char,
) -> c_int {
    let global = unsafe { global_state(vfs) };
    let Some((token, entry)) = (unsafe { token_and_entry(global, name) }) else {
        return ffi::SQLITE_CANTOPEN_FULLPATH;
    };
    let synthetic = format!("{SYNTHETIC_ROOT}/{token}/{}", entry.to_string_lossy());
    let bytes = synthetic.as_bytes();
    if output_length <= 0 || bytes.len() >= output_length as usize {
        return ffi::SQLITE_CANTOPEN_FULLPATH;
    }
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), output.cast::<u8>(), bytes.len());
        *output.add(bytes.len()) = 0;
    }
    ffi::SQLITE_OK
}

unsafe fn global_state<'a>(vfs: *mut ffi::sqlite3_vfs) -> &'a GlobalState {
    unsafe { &*((*vfs).pAppData.cast::<GlobalState>()) }
}

unsafe fn anchored_entry(
    global: &GlobalState,
    name: *const c_char,
) -> Option<(Arc<DirectoryState>, CString)> {
    let (token, entry) = unsafe { token_and_entry(global, name) }?;
    let directory = global.directories.lock().ok()?.get(&token)?.clone();
    Some((directory, entry))
}

unsafe fn token_and_entry(global: &GlobalState, name: *const c_char) -> Option<(String, CString)> {
    let path = Path::new(unsafe { CStr::from_ptr(name) }.to_str().ok()?);
    let entry = path.file_name()?.to_str()?;
    let token = path.parent()?.file_name()?.to_str()?.to_owned();
    let directory = global.directories.lock().ok()?.get(&token)?.clone();
    let database_name = directory.database_name.to_str().ok()?;
    if entry != database_name && !entry.starts_with(&format!("{database_name}-")) {
        return None;
    }
    Some((token, CString::new(entry).ok()?))
}

fn identities() -> &'static Mutex<HashMap<usize, OpenIdentity>> {
    OPEN_IDENTITIES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn entry_has_moved(identity: &OpenIdentity) -> bool {
    let mut descriptor_stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    let mut entry_stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe {
        libc::fstat(
            identity.descriptor.as_raw_fd(),
            descriptor_stat.as_mut_ptr(),
        )
    } == -1
        || unsafe {
            libc::fstatat(
                identity.directory.as_raw_fd(),
                identity.entry.as_ptr(),
                entry_stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } == -1
    {
        return true;
    }
    let descriptor_stat = unsafe { descriptor_stat.assume_init() };
    let entry_stat = unsafe { entry_stat.assume_init() };
    descriptor_stat.st_dev != entry_stat.st_dev || descriptor_stat.st_ino != entry_stat.st_ino
}

fn open_entry(directory: RawFd, entry: &CStr, flags: c_int) -> Result<(File, bool), c_int> {
    let mut open_flags = libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK;
    if flags & ffi::SQLITE_OPEN_READWRITE != 0 {
        open_flags |= libc::O_RDWR;
    } else {
        open_flags |= libc::O_RDONLY;
    }
    let create = flags & ffi::SQLITE_OPEN_CREATE != 0;
    let (descriptor, created) = if create {
        let created = unsafe {
            libc::openat(
                directory,
                entry.as_ptr(),
                open_flags | libc::O_CREAT | libc::O_EXCL,
                0o600,
            )
        };
        if created != -1 {
            if unsafe { libc::fchmod(created, 0o600) } == -1 {
                unsafe { libc::close(created) };
                return Err(ffi::SQLITE_CANTOPEN);
            }
            (created, true)
        } else if io::Error::last_os_error().kind() == io::ErrorKind::AlreadyExists
            && flags & ffi::SQLITE_OPEN_EXCLUSIVE == 0
        {
            (
                unsafe { libc::openat(directory, entry.as_ptr(), open_flags, 0o600) },
                false,
            )
        } else {
            (-1, false)
        }
    } else {
        (
            unsafe { libc::openat(directory, entry.as_ptr(), open_flags, 0o600) },
            false,
        )
    };
    if descriptor == -1 {
        return Err(ffi::SQLITE_CANTOPEN);
    }
    let file = unsafe { File::from_raw_fd(descriptor) };
    if !created
        && flags & ffi::SQLITE_OPEN_MAIN_DB != 0
        && file.metadata().map_err(|_| ffi::SQLITE_CANTOPEN)?.mode() & 0o777 == 0o644
    {
        validate_open_identity(&file)?;
        if unsafe { libc::fchmod(file.as_raw_fd(), 0o600) } == -1 {
            return Err(ffi::SQLITE_CANTOPEN);
        }
        file.sync_all().map_err(|_| ffi::SQLITE_IOERR_FSYNC)?;
    }
    validate_open_file(&file)?;
    Ok((file, created))
}

fn validate_entry_at(directory: &File, entry: &CStr) -> Result<bool, c_int> {
    let descriptor = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            entry.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
            0,
        )
    };
    if descriptor == -1 {
        let error = io::Error::last_os_error();
        return if matches!(
            error.kind(),
            io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
        ) {
            Ok(false)
        } else {
            Err(ffi::SQLITE_CANTOPEN)
        };
    }
    validate_open_file(&unsafe { File::from_raw_fd(descriptor) })?;
    Ok(true)
}

fn validate_open_file(file: &File) -> Result<(), c_int> {
    validate_open_identity(file)?;
    let metadata = file.metadata().map_err(|_| ffi::SQLITE_CANTOPEN)?;
    let mode = metadata.mode() as libc::mode_t;
    if mode & 0o077 != 0 || mode & 0o600 != 0o600 {
        return Err(ffi::SQLITE_CANTOPEN);
    }
    Ok(())
}

fn validate_open_identity(file: &File) -> Result<(), c_int> {
    let metadata = file.metadata().map_err(|_| ffi::SQLITE_CANTOPEN)?;
    let mode = metadata.mode() as libc::mode_t;
    if mode & libc::S_IFMT != libc::S_IFREG
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.nlink() != 1
    {
        return Err(ffi::SQLITE_CANTOPEN);
    }
    Ok(())
}

fn to_rusqlite_conversion_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}

fn sqlite_error(code: c_int, message: &str) -> rusqlite::Error {
    rusqlite::Error::SqliteFailure(ffi::Error::new(code), Some(message.to_owned()))
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use std::fs::File;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::ptr;
    use std::sync::Arc;

    use rusqlite::{Connection, OpenFlags, ffi};
    use tempfile::tempdir;

    use super::{AnchoredVfs, VFS_NAME};

    fn open(directory: &File) -> rusqlite::Result<(Connection, AnchoredVfs)> {
        let vfs = AnchoredVfs::register(directory, "lifesub.sqlite3")?;
        let connection = Connection::open_with_flags_and_vfs(
            vfs.database_path(),
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            vfs.name(),
        )?;
        Ok((connection, vfs))
    }

    #[test]
    fn failed_native_open_is_closed_exactly_once() {
        let root = tempdir().unwrap();
        let directory = File::open(root.path()).unwrap();
        let before = super::FAILED_OPEN_NATIVE_CLOSES.get();
        super::FAIL_AFTER_NATIVE_OPEN.set(true);

        let result = open(&directory);

        assert!(result.is_err());
        assert_eq!(super::FAILED_OPEN_NATIVE_CLOSES.get(), before + 1);
    }

    #[test]
    fn null_named_file_backed_temp_open_is_rejected() {
        let root = tempdir().unwrap();
        let directory = File::open(root.path()).unwrap();
        let (_connection, _vfs) = open(&directory).unwrap();
        let vfs = unsafe { ffi::sqlite3_vfs_find(VFS_NAME.as_ptr()) };
        let mut storage = vec![0_u64; unsafe { (*vfs).szOsFile as usize }.div_ceil(8)];
        let result = unsafe {
            (*vfs).xOpen.unwrap()(
                vfs,
                ptr::null(),
                storage.as_mut_ptr().cast::<ffi::sqlite3_file>(),
                ffi::SQLITE_OPEN_TEMP_DB
                    | ffi::SQLITE_OPEN_READWRITE
                    | ffi::SQLITE_OPEN_CREATE
                    | ffi::SQLITE_OPEN_DELETEONCLOSE,
                ptr::null_mut(),
            )
        };

        assert_eq!(result, ffi::SQLITE_CANTOPEN);
    }

    #[test]
    fn created_database_is_regular_private_and_single_linked() {
        let root = tempdir().unwrap();
        let directory = File::open(root.path()).unwrap();
        let (_connection, _vfs) = open(&directory).unwrap();

        let metadata = root.path().join("lifesub.sqlite3").metadata().unwrap();
        assert!(metadata.is_file());
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        assert_eq!(metadata.nlink(), 1);
    }

    #[test]
    fn permissive_existing_database_is_rejected() {
        let root = tempdir().unwrap();
        let path = root.path().join("lifesub.sqlite3");
        std::fs::write(&path, []).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)).unwrap();
        let directory = File::open(root.path()).unwrap();

        assert!(open(&directory).is_err());
        assert_eq!(path.metadata().unwrap().permissions().mode() & 0o777, 0o666);
    }

    #[test]
    fn legacy_readable_main_database_is_hardened_by_descriptor() {
        let root = tempdir().unwrap();
        let path = root.path().join("lifesub.sqlite3");
        std::fs::write(&path, []).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let directory = File::open(root.path()).unwrap();

        let (_connection, _vfs) = open(&directory).unwrap();

        assert_eq!(path.metadata().unwrap().permissions().mode() & 0o777, 0o600);
    }

    #[test]
    fn fifo_database_entry_is_rejected() {
        let root = tempdir().unwrap();
        let path =
            CString::new(root.path().join("lifesub.sqlite3").as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        let directory = File::open(root.path()).unwrap();

        assert!(open(&directory).is_err());
    }

    #[test]
    fn directory_database_entry_is_rejected() {
        let root = tempdir().unwrap();
        std::fs::create_dir(root.path().join("lifesub.sqlite3")).unwrap();
        let directory = File::open(root.path()).unwrap();

        assert!(open(&directory).is_err());
    }

    #[test]
    fn hard_linked_database_entry_is_rejected() {
        let root = tempdir().unwrap();
        let database = root.path().join("lifesub.sqlite3");
        std::fs::write(&database, []).unwrap();
        std::fs::set_permissions(&database, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::hard_link(&database, root.path().join("alias.sqlite3")).unwrap();
        let directory = File::open(root.path()).unwrap();

        assert!(open(&directory).is_err());
    }

    #[test]
    fn dropping_prepared_token_revokes_later_resolution() {
        let root = tempdir().unwrap();
        let directory = File::open(root.path()).unwrap();
        let vfs = AnchoredVfs::register(&directory, "lifesub.sqlite3").unwrap();
        let database_path = vfs.database_path().to_owned();
        let vfs_name = vfs.name().to_owned();
        drop(vfs);

        let result = Connection::open_with_flags_and_vfs(
            database_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
            &*vfs_name,
        );

        assert!(result.is_err());
        assert!(!root.path().join("lifesub.sqlite3").exists());
    }

    #[test]
    fn concurrent_anchored_connections_do_not_cross_tokens() {
        let roots = (0..16).map(|_| tempdir().unwrap()).collect::<Vec<_>>();
        let paths = roots
            .iter()
            .map(|root| root.path().to_path_buf())
            .collect::<Vec<_>>();
        let paths = Arc::new(paths);
        let threads = (0..16)
            .map(|index| {
                let paths = Arc::clone(&paths);
                std::thread::spawn(move || {
                    let directory = File::open(&paths[index]).unwrap();
                    let (connection, _vfs) = open(&directory).unwrap();
                    connection
                        .execute_batch(&format!(
                            "CREATE TABLE owner(value INTEGER); INSERT INTO owner VALUES({index});"
                        ))
                        .unwrap();
                })
            })
            .collect::<Vec<_>>();
        for thread in threads {
            thread.join().unwrap();
        }
        for (index, path) in paths.iter().enumerate() {
            let connection = Connection::open(path.join("lifesub.sqlite3")).unwrap();
            assert_eq!(
                connection
                    .query_row("SELECT value FROM owner", [], |row| row.get::<_, usize>(0))
                    .unwrap(),
                index
            );
        }
    }

    #[test]
    fn main_inode_replacement_is_reported_as_database_moved() {
        let root = tempdir().unwrap();
        let directory = File::open(root.path()).unwrap();
        let (connection, _vfs) = open(&directory).unwrap();
        connection
            .execute_batch("CREATE TABLE evidence(value INTEGER); INSERT INTO evidence VALUES(1)")
            .unwrap();
        let database = root.path().join("lifesub.sqlite3");
        std::fs::rename(&database, root.path().join("held.sqlite3")).unwrap();
        std::fs::write(&database, []).unwrap();
        std::fs::set_permissions(&database, std::fs::Permissions::from_mode(0o600)).unwrap();

        let error = connection
            .execute("INSERT INTO evidence VALUES(2)", [])
            .unwrap_err();

        assert_eq!(
            error.sqlite_error_code(),
            Some(rusqlite::ErrorCode::ReadOnly)
        );
        assert_eq!(database.metadata().unwrap().len(), 0);
    }

    #[test]
    fn rollback_journal_stays_with_renamed_anchored_directory() {
        let parent = tempdir().unwrap();
        let data = parent.path().join("data");
        let held = parent.path().join("held");
        std::fs::create_dir(&data).unwrap();
        let directory = File::open(&data).unwrap();
        let (connection, _vfs) = open(&directory).unwrap();
        connection
            .execute_batch("PRAGMA journal_mode=DELETE; CREATE TABLE evidence(value INTEGER)")
            .unwrap();
        std::fs::rename(&data, &held).unwrap();
        std::fs::create_dir(&data).unwrap();

        connection
            .execute_batch("BEGIN IMMEDIATE; INSERT INTO evidence VALUES(1); COMMIT")
            .unwrap();

        assert!(std::fs::read_dir(&data).unwrap().next().is_none());
        assert_eq!(
            Connection::open(held.join("lifesub.sqlite3"))
                .unwrap()
                .query_row("SELECT COUNT(*) FROM evidence", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn unix_excl_wal_is_shared_by_two_connections_without_physical_shm() {
        let parent = tempdir().unwrap();
        let data = parent.path().join("data");
        let held = parent.path().join("held");
        std::fs::create_dir(&data).unwrap();
        let directory = File::open(&data).unwrap();
        let vfs = AnchoredVfs::register(&directory, "lifesub.sqlite3").unwrap();
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let first =
            Connection::open_with_flags_and_vfs(vfs.database_path(), flags, vfs.name()).unwrap();
        let second =
            Connection::open_with_flags_and_vfs(vfs.database_path(), flags, vfs.name()).unwrap();
        assert_eq!(
            first
                .query_row("PRAGMA journal_mode=WAL", [], |row| row.get::<_, String>(0))
                .unwrap(),
            "wal"
        );
        first
            .execute_batch("CREATE TABLE evidence(value INTEGER); INSERT INTO evidence VALUES(1)")
            .unwrap();
        assert_eq!(
            second
                .query_row("SELECT COUNT(*) FROM evidence", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        std::fs::rename(&data, &held).unwrap();
        std::fs::create_dir(&data).unwrap();

        second
            .execute("INSERT INTO evidence VALUES(2)", [])
            .unwrap();

        assert!(std::fs::read_dir(&data).unwrap().next().is_none());
        assert!(held.join("lifesub.sqlite3-wal").exists());
        assert!(!held.join("lifesub.sqlite3-shm").exists());
    }
}
