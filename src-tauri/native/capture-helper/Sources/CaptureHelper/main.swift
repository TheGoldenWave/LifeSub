import Foundation

// The signed bundle launches this executable. Task 6 wires the authenticated
// bootstrap channel and command loop; exiting here is fail-closed meanwhile.
FileHandle.standardError.write(Data("capture helper is not supervised\n".utf8))
exit(EXIT_FAILURE)
