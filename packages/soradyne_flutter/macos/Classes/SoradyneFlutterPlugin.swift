// Placeholder — soradyne_flutter is an FFI-only plugin on macOS.
// The Rust dylib is loaded directly by Dart via DynamicLibrary.open().
// No method channel or platform view registration is needed here.
import FlutterMacOS

public class SoradyneFlutterPlugin: NSObject, FlutterPlugin {
    public static func register(with registrar: FlutterPluginRegistrar) {
        // FFI-only: nothing to register.
    }
}
