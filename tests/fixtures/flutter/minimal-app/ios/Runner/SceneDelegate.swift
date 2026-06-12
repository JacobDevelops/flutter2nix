import Flutter
import UIKit

// Storyboard-free bootstrap: the e2e fixture compiles without ibtool/actool,
// which need a CoreSimulator user context unavailable to Nix build users.
class SceneDelegate: FlutterSceneDelegate {
  override func scene(
    _ scene: UIScene,
    willConnectTo session: UISceneSession,
    options connectionOptions: UIScene.ConnectionOptions
  ) {
    super.scene(scene, willConnectTo: session, options: connectionOptions)
    guard let windowScene = scene as? UIWindowScene else { return }
    let window = UIWindow(windowScene: windowScene)
    window.rootViewController = FlutterViewController(project: nil, nibName: nil, bundle: nil)
    window.makeKeyAndVisible()
    self.window = window
  }
}
