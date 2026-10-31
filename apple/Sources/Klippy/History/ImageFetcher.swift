import Foundation

/// Downloads a copy's images for the Rust core, which cleans them. Only
/// https; no cookies, cache or saved logins; each capped in size and time.
enum ImageFetcher {
    /// Matches `clean::MAX_IMAGE_BYTES` in the core, which checks it again.
    private static let maxBytes = 5 * 1024 * 1024

    private static let session: URLSession = {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 10
        configuration.httpShouldSetCookies = false
        configuration.httpCookieStorage = nil
        configuration.urlCache = nil
        configuration.urlCredentialStorage = nil
        return URLSession(configuration: configuration)
    }()

    /// One result per URL, in order; nil where the download failed.
    static func fetch(_ urls: [String]) async -> [[UInt8]?] {
        await withTaskGroup(of: (Int, [UInt8]?).self) { group in
            for (index, url) in urls.enumerated() {
                group.addTask { (index, await download(url)) }
            }
            var results = [[UInt8]?](repeating: nil, count: urls.count)
            for await (index, data) in group { results[index] = data }
            return results
        }
    }

    private static func download(_ string: String) async -> [UInt8]? {
        guard let url = URL(string: string), url.scheme?.lowercased() == "https",
              let (bytes, response) = try? await session.bytes(from: url),
              let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode),
              http.expectedContentLength <= Int64(maxBytes)
        else { return nil }
        var data = [UInt8]()
        do {
            for try await byte in bytes {
                data.append(byte)
                if data.count > maxBytes { return nil }
            }
        } catch {
            return nil
        }
        return data
    }
}
