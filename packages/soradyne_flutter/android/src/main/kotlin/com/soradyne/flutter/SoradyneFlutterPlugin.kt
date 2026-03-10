package com.soradyne.flutter

import android.content.Context
import android.util.Log
import io.flutter.embedding.engine.plugins.FlutterPlugin

/**
 * Android entry point for the soradyne_flutter FFI plugin.
 *
 * Flutter loads `libsoradyne.so` automatically via the `ffiPlugin: true`
 * declaration in pubspec.yaml. This class piggybacks on the plugin lifecycle
 * to inject the Android [Context] into Rust before any pairing calls are made.
 *
 * The native side stores it in a global ref inside `PAIRING_BRIDGE` and uses
 * it when `soradyne_pairing_start_invite` needs a `BluetoothLeAdvertiser`.
 */
class SoradyneFlutterPlugin : FlutterPlugin {

    companion object {
        init {
            // Load the Rust library here so nativeSetContext is resolvable when
            // onAttachedToEngine fires — before Flutter's FFI loader has run.
            System.loadLibrary("soradyne")
        }

        /**
         * Passes the Android Context to Rust.
         * Implemented in `src/ffi/pairing_bridge.rs` as
         * `Java_com_soradyne_flutter_SoradyneFlutterPlugin_nativeSetContext`.
         *
         * Returns 0 on success, -1 on error (bridge not yet initialised).
         */
        @JvmStatic
        private external fun nativeSetContext(context: Context): Int
    }

    override fun onAttachedToEngine(binding: FlutterPlugin.FlutterPluginBinding) {
        // libsoradyne.so is already mapped by Flutter's FFI loader at this point.
        // Hand the application context to Rust so it can open a BluetoothGattServer
        // and BluetoothLeAdvertiser when the inviter flow starts.
        Log.d("SoradynePlugin", "onAttachedToEngine: calling nativeSetContext")
        val result = nativeSetContext(binding.applicationContext)
        Log.d("SoradynePlugin", "nativeSetContext returned $result")
    }

    override fun onDetachedFromEngine(binding: FlutterPlugin.FlutterPluginBinding) {
        // Nothing to clean up on the Rust side — the bridge owns its own lifetime.
    }
}
