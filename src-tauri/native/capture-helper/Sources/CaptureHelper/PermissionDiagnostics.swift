@preconcurrency import AVFoundation
import CaptureProtocol
import CoreGraphics
import Foundation

protocol PermissionStateProviding: Sendable {
    func currentPermissionState() -> PermissionStateMessage
}

struct LivePermissionStateProvider: PermissionStateProviding {
    func currentPermissionState() -> PermissionStateMessage {
        PermissionStateMessage(
            microphone: microphonePermission(),
            screenRecording: CGPreflightScreenCaptureAccess() ? .granted : .notGranted
        )
    }

    private func microphonePermission() -> Permission {
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized: .granted
        case .notDetermined: .notDetermined
        case .denied: .denied
        case .restricted: .restricted
        @unknown default: .restricted
        }
    }
}
