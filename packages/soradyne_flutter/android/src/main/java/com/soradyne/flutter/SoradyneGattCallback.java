package com.soradyne.flutter;

import android.bluetooth.BluetoothDevice;
import android.bluetooth.BluetoothGatt;
import android.bluetooth.BluetoothGattCharacteristic;
import android.bluetooth.BluetoothGattDescriptor;
import android.bluetooth.BluetoothGattServer;
import android.bluetooth.BluetoothGattServerCallback;
import android.bluetooth.BluetoothProfile;
import android.util.Log;

/**
 * GATT server callback that bridges BLE events to Rust via JNI.
 *
 * Constructed before the GATT server is opened (to break the chicken-and-egg
 * dependency), then wired up via {@link #setGattServer} once the server handle
 * is available.
 *
 * The native methods are implemented in
 * {@code packages/soradyne_core/src/ble/android_peripheral.rs}.
 */
public class SoradyneGattCallback extends BluetoothGattServerCallback {

    static {
        System.loadLibrary("soradyne");
    }

    private static final String TAG = "SoradyneGattCallback";
    // CCCD descriptor UUID (notify enable/disable)
    private static final String CCCD_UUID = "00002902-0000-1000-8000-00805f9b34fb";

    // -----------------------------------------------------------------------
    // Native method declarations (implemented in Rust)
    // -----------------------------------------------------------------------

    /** Called when a BLE central has enabled notifications and is ready. */
    private static native void nativeOnConnected(BluetoothDevice device,
                                                  BluetoothGattServer server);

    /** Called when a central writes to the envelope characteristic. */
    private static native void nativeOnWrite(String deviceKey, byte[] data);

    /** Called when a central disconnects. */
    private static native void nativeOnDisconnected(String deviceKey);

    /**
     * Called by Android after a notification has been transmitted to the central.
     * Rust uses this as a flow-control signal between multi-chunk sends.
     */
    private static native void nativeOnNotificationSent(String deviceKey, int status);

    // -----------------------------------------------------------------------
    // Constructor + server wiring
    // -----------------------------------------------------------------------

    private BluetoothGattServer gattServer;

    /** No-arg constructor — call {@link #setGattServer} immediately after openGattServer. */
    public SoradyneGattCallback() {}

    /** Wire up the server handle after it has been obtained from openGattServer. */
    public void setGattServer(BluetoothGattServer server) {
        this.gattServer = server;
    }

    // -----------------------------------------------------------------------
    // BluetoothGattServerCallback overrides
    // -----------------------------------------------------------------------

    @Override
    public void onConnectionStateChange(BluetoothDevice device, int status, int newState) {
        Log.i(TAG, "onConnectionStateChange: device=" + device.getAddress()
                + " status=" + status + " newState=" + newState);
        // NOTE: nativeOnConnected is called from onDescriptorWriteRequest (CCCD=0x01),
        // not here. This ensures notifications are enabled before the inviter calls
        // notifyCharacteristicChanged for the first protocol message.
        if (newState == BluetoothProfile.STATE_DISCONNECTED) {
            Log.i(TAG, "disconnected: " + device.getAddress());
            nativeOnDisconnected(device.getAddress());
        }
    }

    @Override
    public void onMtuChanged(BluetoothDevice device, int mtu) {
        Log.i(TAG, "onMtuChanged: device=" + device.getAddress() + " mtu=" + mtu);
    }

    @Override
    public void onNotificationSent(BluetoothDevice device, int status) {
        Log.i(TAG, "onNotificationSent: device=" + device.getAddress() + " status=" + status);
        nativeOnNotificationSent(device.getAddress(), status);
    }

    @Override
    public void onCharacteristicWriteRequest(BluetoothDevice device,
                                              int requestId,
                                              BluetoothGattCharacteristic characteristic,
                                              boolean preparedWrite,
                                              boolean responseNeeded,
                                              int offset,
                                              byte[] value) {
        Log.i(TAG, "onCharacteristicWriteRequest: device=" + device.getAddress()
                + " len=" + (value != null ? value.length : 0));
        nativeOnWrite(device.getAddress(), value);
        if (responseNeeded && gattServer != null) {
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
        Log.i(TAG, "onDescriptorWriteRequest: device=" + device.getAddress()
                + " uuid=" + descriptor.getUuid()
                + " value=" + (value != null && value.length >= 1 ? value[0] : "null"));
        if (responseNeeded && gattServer != null) {
            gattServer.sendResponse(device, requestId,
                    BluetoothGatt.GATT_SUCCESS, 0, null);
        }
        // Start the protocol only after the central has enabled notifications
        // (CCCD = 0x01). This prevents notifyCharacteristicChanged from being
        // called before the central is subscribed, which would silently drop data.
        if (descriptor.getUuid().toString().equalsIgnoreCase(CCCD_UUID)
                && value != null && value.length >= 1 && value[0] == 0x01) {
            Log.i(TAG, "CCCD enabled by " + device.getAddress() + " — starting protocol");
            nativeOnConnected(device, gattServer);
        }
    }
}
