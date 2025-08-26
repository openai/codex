import Cocoa
import WebKit

final class AppDelegate: NSObject, NSApplicationDelegate, WKNavigationDelegate, NSWindowDelegate {
    private var didComplete = false
    private var window: NSWindow!
    private var webView: WKWebView!
    private var authorizeURL: URL!
    private var expectedState: String!

    func applicationDidFinishLaunching(_ notification: Notification) {
        buildMenus()
        let args = CommandLine.arguments
        guard let urlIdx = args.firstIndex(of: "--authorize-url").flatMap({ idx in
            idx + 1 < args.count ? idx + 1 : nil
        }), let stateIdx = args.firstIndex(of: "--state").flatMap({ idx in
            idx + 1 < args.count ? idx + 1 : nil }) else {
            fputs("usage: codex-auth-helper --authorize-url <URL> --state <STATE>\n", stderr)
            NSApp.terminate(nil)
            return
        }
        guard let authURL = URL(string: args[urlIdx]) else {
            fputs("invalid authorize url\n", stderr)
            NSApp.terminate(nil)
            return
        }
        self.authorizeURL = authURL
        self.expectedState = args[stateIdx]

        let config = WKWebViewConfiguration()
        config.websiteDataStore = .nonPersistent()

        webView = WKWebView(frame: .zero, configuration: config)
        webView.navigationDelegate = self

        window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 720, height: 820),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.center()
        window.title = "Codex – Sign in to OpenAI"
        window.contentView = webView
        window.delegate = self
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(webView)
        NSApp.activate(ignoringOtherApps: true)

        webView.load(URLRequest(url: authorizeURL))
    }

    // Close behavior: treat closing the window as an abort and exit non‑zero.
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
    func windowWillClose(_ notification: Notification) { exit(didComplete ? 0 : 2) }

    private func buildMenus() {
        let mainMenu = NSMenu()
        let appName = ProcessInfo.processInfo.processName

        // App menu
        let appMenuItem = NSMenuItem()
        let appMenu = NSMenu(title: appName)
        appMenu.addItem(withTitle: "About \(appName)", action: #selector(NSApplication.orderFrontStandardAboutPanel(_:)), keyEquivalent: "")
        appMenu.addItem(NSMenuItem.separator())
        appMenu.addItem(withTitle: "Quit \(appName)", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        appMenuItem.submenu = appMenu

        // Edit menu (enables Cmd+C/V, etc.)
        let editMenuItem = NSMenuItem()
        let editMenu = NSMenu(title: "Edit")
        editMenu.addItem(withTitle: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        editMenu.addItem(withTitle: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        editMenu.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        editMenu.addItem(withTitle: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
        editMenuItem.submenu = editMenu

        mainMenu.addItem(appMenuItem)
        mainMenu.addItem(editMenuItem)
        NSApp.mainMenu = mainMenu
    }

    func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        guard let url = navigationAction.request.url else { decisionHandler(.allow); return }
        if url.scheme == "http", (url.host == "localhost" || url.host == "127.0.0.1"), url.path.hasPrefix("/auth/callback") {
            if let comps = URLComponents(url: url, resolvingAgainstBaseURL: false) {
                let qp = Dictionary(uniqueKeysWithValues: (comps.queryItems ?? []).map { ($0.name, $0.value ?? "") })
                let code = qp["code"] ?? ""
                let state = qp["state"] ?? ""
                if code.isEmpty { fputs("missing authorization code\n", stderr) }
                else if state != expectedState { fputs("state mismatch\n", stderr) }
                else {
                    didComplete = true
                    let payload = "{\"code\":\"\(code)\",\"state\":\"\(state)\"}\n"
                    if let data = payload.data(using: .utf8) { FileHandle.standardOutput.write(data) }
                }
            }
            decisionHandler(.cancel)
            NSApp.terminate(nil)
            return
        }
        decisionHandler(.allow)
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        fputs("navigation error: \(error)\n", stderr)
    }
    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        fputs("provisional nav error: \(error)\n", stderr)
    }
}

let app = NSApplication.shared
app.setActivationPolicy(.regular)
let delegate = AppDelegate()
app.delegate = delegate
app.run()
