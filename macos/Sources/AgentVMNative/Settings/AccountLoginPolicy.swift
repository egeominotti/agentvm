import Foundation

struct NativeAccountLoginSession: Equatable, Sendable {
    let id: String
    let verificationURL: URL
    let userCode: String?
    let expiresAt: Date
}

enum NativeAccountLoginPhase: Equatable, Sendable {
    case pending
    case connected
    case failed(String)
}

enum NativeAccountPollDecision: Equatable, Sendable {
    case request
    case paused
    case expired
    case limitReached
    case finished
}

enum NativeAccountLoginPolicy {
    static func session(response: JSON, now: Date) throws -> NativeAccountLoginSession {
        let id = response["id"].text
        let seconds = response["expires_in"].number
        guard !id.isEmpty, id.utf8.allSatisfy({ (48...57).contains($0) || (65...90).contains($0)
                  || (97...122).contains($0) || $0 == 45 || $0 == 95 }),
              seconds.isFinite, seconds > 0, now.timeIntervalSince1970.isFinite,
              now.addingTimeInterval(seconds).timeIntervalSince1970.isFinite else {
            throw NativeAPIError(status: nil, message: "The server returned an invalid sign-in session.")
        }
        guard let url = URLComponents(string: response["verification_url"].text),
              url.scheme?.lowercased() == "https", let host = url.host, !host.isEmpty,
              url.user == nil, url.password == nil, let destination = url.url else {
            throw NativeAPIError(status: nil, message: "The server returned an invalid sign-in verification page.")
        }
        let code = response["user_code"].text.trimmingCharacters(in: .whitespacesAndNewlines)
        return .init(id: id, verificationURL: destination, userCode: code.isEmpty ? nil : code,
                     expiresAt: now.addingTimeInterval(seconds))
    }

    static func phase(response: JSON, id: String) -> NativeAccountLoginPhase {
        guard response["id"].text == id else { return .failed("The server returned a different sign-in session. Start again.") }
        switch response["status"].text {
        case "pending": return .pending
        case "connected": return .connected
        case "failed":
            let error = response["error"].text
            return .failed(error.isEmpty ? "Account sign-in failed. Try again." : error)
        default: return .failed("The server returned an unknown sign-in status. Start again.")
        }
    }

    static func pollDecision(session: NativeAccountLoginSession, phase: NativeAccountLoginPhase, polls: Int,
                             now: Date, active: Bool) -> NativeAccountPollDecision {
        guard phase == .pending else { return .finished }
        guard active else { return .paused }
        if now >= session.expiresAt { return .expired }
        if polls >= 300 { return .limitReached }
        return .request
    }
}
