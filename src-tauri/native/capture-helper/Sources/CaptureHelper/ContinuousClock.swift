import Darwin
import Foundation

enum CaptureContinuousClock {
    private static let timebase: mach_timebase_info_data_t = {
        var info = mach_timebase_info_data_t()
        mach_timebase_info(&info)
        return info
    }()

    static func nanoseconds(fromMachTicks ticks: UInt64) -> UInt64 {
        let (scaled, overflow) = ticks.multipliedReportingOverflow(by: UInt64(timebase.numer))
        guard !overflow, timebase.denom != 0 else { return UInt64.max }
        return scaled / UInt64(timebase.denom)
    }

    static func nowNanoseconds() -> UInt64 {
        nanoseconds(fromMachTicks: mach_absolute_time())
    }
}
