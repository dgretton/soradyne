package com.soradyne.flutter;

import android.bluetooth.BluetoothDevice;
import android.bluetooth.BluetoothGatt;
import android.bluetooth.BluetoothGattCharacteristic;
import android.bluetooth.BluetoothGattDescriptor;
import android.bluetooth.BluetoothGattServer;
import android.bluetooth.BluetoothGattServerCallback;
import android.bluetooth.BluetoothProfile;
import android.bluetooth.le.AdvertiseCallback;
import android.bluetooth.le.AdvertiseSettings;

/**
 * GATT server callback that bridges BLE events to Rust via JNI.
 *
 * Extends {@link BluetoothGattServerCallback} to receive GATT events and
 * extends {@link AdvertiseCallback} so the same instance can be passed to
 * {@code BluetoothLeAdvertiser.startAdvertising()}.
 *
 * The native methods are implemented in
 * {@code packages/soradyne_core/src/ble/android_peripheral.rs}.
 */
public class SoradyneGattCallback extends BluetoothGattServerCallback {

    static {
        System.loadLibrary("soradyne");
    }

    // -----------------------------------------------------------------------
    // Native method declarations (implemented in Rust)
    // -----------------------------------------------------------------------

    /** Called when a BLE central device connects. */
    private static native void nativeOnConnected(BluetoothDevice device,
                                                  BluetoothGattServer server);

    /** Called when a central writes to the envelope characteristic. */
    private static native void nativeOnWrite(String deviceKey, byte[] data);

    /** Called when a central disconnects. */
    private static native void nativeOnDisconnected(String deviceKey);

    // -----------------------------------------------------------------------
    // Constructor
    // -----------------------------------------------------------------------

    private final BluetoothGattServer gattServer;

    public SoradyneGattCallback(BluetoothGattServer server) {
        this.gattServer = server;
    }

    // -----------------------------------------------------------------------
    // BluetoothGattServerCallback overrides
    // -----------------------------------------------------------------------

    @Override
    public void onConnectionStateChange(BluetoothDevice device, int status, int newState) {
        if (newState == BluetoothProfile.STATE_CONNECTED) {
            nativeOnConnected(device, gattServer);
        } else if (newState == BluetoothProfile.STATE_DISCONNECTED) {
            nativeOnDisconnected(device.getAddress());
        }
    }

    @Override
    public void onCharacteristicWriteRequest(BluetoothDevice device,
                                              int requestId,
                                              BluetoothGattCharacteristic characteristic,
                                              boolean preparedWrite,
                                              boolean responseNeeded,
                                              int offset,
                                              byte[] value) {
        nativeOnWrite(device.getAddress(), value);
        if (responseNeeded) {
            gattServer.sendResponse(device, requestId,
                    BluetoothGatt.GATT_SUCCESS, 0, null);
        }
    }

    @Override
    public void onDescriptorWriteRequest(BluetoothDevice device,
                                          int requestId,
                                          BluetoothGattDescriptor descriptor,
                                          boolean preparedWrite,
                                          boolean responseNeeded,
                                          int offset,
                                          byte[] value) {
        // Accept CCCD enable/disable writes unconditionally.
        if (responseNeeded) {
            gattServer.sendResponse(device, requestId,
                    BluetoothGatt.GATT_SUCCESS, 0, null);
        }
    }

    // -----------------------------------------------------------------------
    // AdvertiseCallback methods (no-op stubs; errors surface via logcat)
    // -----------------------------------------------------------------------

    public void onStartSuccess(AdvertiseSettings settingsInEffect) {
        // Advertising started successfully — nothing to do.
    }

    public void onStartFailure(int errorCode) {
        android.util.Log.e("SoradyneGattCallback",
                "BLE advertising failed: errorCode=" + errorCode);
    }
}
