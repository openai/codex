import SwiftUI

let SERVER_URL = "https://ip.rootnet.in"

struct ContentView: View {
    var body: some View {
        Text("Hello World!")
            .padding()
            .onAppear(perform: openBrowser)
    }
    
    private func openBrowser() {
        let _ = TestGlueInterface.testOpenBrowser(url: SERVER_URL)
    }
}

struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
        ContentView()
    }
}
