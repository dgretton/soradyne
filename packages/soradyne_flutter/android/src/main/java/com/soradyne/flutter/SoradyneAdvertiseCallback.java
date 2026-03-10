package com.soradyne.flutter;

import android.bluetooth.le.AdvertiseCallback;
import android.bluetooth.le.AdvertiseSettings;
import android.util.Log;

/**
 * Minimal AdvertiseCallback used with BluetoothLeAdvertiser.startAdvertising().
 *
 * Results are logged to logcat. The Rust layer does not need start/stop
 * notifications for the pairing flow — advertising is fire-and-forget until
 * a central connects to the GATT server.
 */
public class SoradyneAdvertiseCallback extends AdvertiseCallback {

    private static final String TAG = "SoradyneAdvertise";

    @Override
    public void onStartSuccess(AdvertiseSettings settingsInEffect) {
        Log.i(TAG, "BLE advertising started successfully");
    }

    @Override
    public void onStartFailure(int errorCode) {
        Log.e(TAG, "BLE advertising failed: errorCode=" + errorCode);
    }
}
