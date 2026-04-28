package com.soradyne.demo

import com.soradyne.flutter.SoradyneFlutterPlugin
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine

class MainActivity : FlutterActivity() {
    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        // soradyne_flutter has ffiPlugin:true + pluginClass, but Flutter's tooling
        // drops the class registration when both are set.  Register manually so
        // SoradyneFlutterPlugin.onAttachedToEngine fires and injects the Android
        // Context into Rust before soradyne_pairing_start_invite is called.
        flutterEngine.plugins.add(SoradyneFlutterPlugin())
    }
}
