import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'package:flutter/foundation.dart';
import 'package:path_provider/path_provider.dart';
import '../ffi/pairing_bindings.dart';

/// Typed representation of the pairing engine state.
enum PairingStateType { idle, awaitingVerification, transferring, complete, failed }

/// Parsed pairing state from FFI JSON.
class PairingStateData {
  final PairingStateType type;
  final String? pin;
  final String? capsuleId;
  final String? peerDeviceId;
  final String? reason;

  const PairingStateData({
    required this.type,
    this.pin,
    this.capsuleId,
    this.peerDeviceId,
    this.reason,
  });

  factory PairingStateData.fromJson(Map<String, dynamic> json) {
    final state = json['state'] as String? ?? 'idle';
    switch (state) {
      case 'awaiting_verification':
        return PairingStateData(
          type: PairingStateType.awaitingVerification,
          pin: json['pin'] as String?,
        );
      case 'transferring':
        return const PairingStateData(type: PairingStateType.transferring);
      case 'complete':
        return PairingStateData(
          type: PairingStateType.complete,
          capsuleId: json['capsule_id'] as String?,
          peerDeviceId: json['peer_device_id'] as String?,
        );
      case 'failed':
        return PairingStateData(
          type: PairingStateType.failed,
          reason: json['reason'] as String?,
        );
      default:
        return const PairingStateData(type: PairingStateType.idle);
    }
  }
}

/// Parsed capsule data from FFI JSON.
class CapsuleData {
  final String id;
  final String name;
  final String createdAt;
  final int pieceCount;
  final bool isActive;
  final List<PieceData> pieces;

  CapsuleData({
    required this.id,
    required this.name,
    required this.createdAt,
    required this.pieceCount,
    required this.isActive,
    required this.pieces,
  });

  factory CapsuleData.fromJson(Map<String, dynamic> json) {
    final piecesJson = json['pieces'] as List<dynamic>? ?? [];
    return CapsuleData(
      id: json['id'] as String? ?? '',
      name: json['name'] as String? ?? '',
      createdAt: json['created_at'] as String? ?? '',
      pieceCount: json['piece_count'] as int? ?? piecesJson.length,
      isActive: json['is_active'] as bool? ?? true,
      pieces: piecesJson
          .map((p) => PieceData.fromJson(p as Map<String, dynamic>))
          .toList(),
    );
  }
}

/// Parsed piece data from FFI JSON.
class PieceData {
  final String deviceId;
  final String name;
  final String role;
  final String addedAt;

  PieceData({
    required this.deviceId,
    required this.name,
    required this.role,
    required this.addedAt,
  });

  factory PieceData.fromJson(Map<String, dynamic> json) {
    return PieceData(
      deviceId: json['device_id'] as String? ?? '',
      name: json['name'] as String? ?? '',
      role: json['role'] as String? ?? 'Full',
      addedAt: json['added_at'] as String? ?? '',
    );
  }
}

/// Service wrapping the pairing FFI with state polling.
class PairingService extends ChangeNotifier {
  late final PairingBindings _bindings;

  List<CapsuleData> _capsules = [];
  PairingStateData _pairingState =
      const PairingStateData(type: PairingStateType.idle);
  bool _initialized = false;
  bool _polling = false;
  String? _error;
  Timer? _pollTimer;

  List<CapsuleData> get capsules => _capsules;
  PairingStateData get pairingState => _pairingState;
  bool get initialized => _initialized;
  String? get error => _error;

  /// The raw FFI bindings — used by other services (e.g. NotesService) to
  /// share the DynamicLibrary without opening it a second time.
  PairingBindings? get bindings => _initialized ? _bindings : null;

  /// This device's UUID as a string, or null before init completes.
  String? get deviceId => _initialized ? _bindings.getDeviceId() : null;

  PairingService() {
    _initializeBindings();
  }

  Future<void> _initializeBindings() async {
    // Yield immediately so this always runs after the first build frame.
    // Without this, notifyListeners() could be called during build (e.g. on
    // macOS where no await precedes it), which is a framework violation that
    // silently corrupts gesture recogniser state.
    await Future.microtask(() {});
    try {
      debugPrint('PairingService: creating bindings...');
      _bindings = PairingBindings();

      // Resolve a writable data directory appropriate for the platform.
      // On Android, getApplicationDocumentsDirectory() returns the app's
      // internal storage — the only place the process is allowed to write.
      // On other platforms we let Rust pick its own default (pass null).
      String? dataDir;
      if (Platform.isAndroid || Platform.isIOS) {
        final dir = await getApplicationDocumentsDirectory();
        dataDir = dir.path;
        debugPrint('PairingService: using dataDir=$dataDir');
      }

      final result = _bindings.init(dataDir: dataDir);
      if (result == 0) {
        _initialized = true;
        debugPrint('PairingService: initialized successfully');
        refreshCapsules();
        notifyListeners();
      } else {
        _error = 'Failed to initialize pairing bridge (code: $result)';
        debugPrint('PairingService: init failed: $_error');
        notifyListeners();
      }
    } catch (e) {
      _error = 'FFI initialization error: $e';
      debugPrint('PairingService: $_error');
      notifyListeners();
    }
  }

  /// Refresh the capsule list from disk.
  void refreshCapsules() {
    if (!_initialized) return;
    try {
      final json = _bindings.listCapsules();
      final list = jsonDecode(json);
      if (list is List) {
        _capsules = list
            .map((e) => CapsuleData.fromJson(e as Map<String, dynamic>))
            .toList();
        _error = null;
      } else if (list is Map && list.containsKey('error')) {
        _error = list['error'] as String?;
      }
      notifyListeners();
    } catch (e) {
      debugPrint('PairingService.refreshCapsules error: $e');
      _error = 'Error listing capsules: $e';
      notifyListeners();
    }
  }

  /// Create a new capsule.
  String? createCapsule(String name) {
    if (!_initialized) return null;
    try {
      final json = _bindings.createCapsule(name);
      final result = jsonDecode(json) as Map<String, dynamic>;
      if (result.containsKey('error')) {
        _error = result['error'] as String?;
        notifyListeners();
        return null;
      }
      refreshCapsules();
      return result['capsule_id'] as String?;
    } catch (e) {
      debugPrint('PairingService.createCapsule error: $e');
      return null;
    }
  }

  /// Delete a capsule permanently. Returns true on success.
  bool deleteCapsule(String capsuleId) {
    if (!_initialized) return false;
    try {
      final result = _bindings.deleteCapsule(capsuleId);
      if (result == 0) {
        refreshCapsules();
        return true;
      }
      return false;
    } catch (e) {
      debugPrint('PairingService.deleteCapsule error: $e');
      return false;
    }
  }

  /// Get detailed capsule info.
  CapsuleData? getCapsule(String capsuleId) {
    if (!_initialized) return null;
    try {
      final json = _bindings.getCapsule(capsuleId);
      final result = jsonDecode(json) as Map<String, dynamic>;
      if (result.containsKey('error')) return null;
      return CapsuleData.fromJson(result);
    } catch (e) {
      debugPrint('PairingService.getCapsule error: $e');
      return null;
    }
  }

  /// Start polling pairing state at 500ms intervals.
  void _startPolling() {
    if (_polling) return;
    _polling = true;
    _pollTimer = Timer.periodic(const Duration(milliseconds: 500), (_) {
      _pollState();
    });
  }

  /// Stop polling.
  void _stopPolling() {
    _polling = false;
    _pollTimer?.cancel();
    _pollTimer = null;
  }

  /// Poll the pairing state once.
  void _pollState() {
    if (!_initialized) return;
    // Drain any BLE debug log lines from Rust (invisible on macOS otherwise).
    final bleLog = _bindings.bleDebug();
    if (bleLog.isNotEmpty) debugPrint('BLE: $bleLog');
    try {
      final json = _bindings.getState();
      final result = jsonDecode(json) as Map<String, dynamic>;
      final newState = PairingStateData.fromJson(result);

      if (newState.type != _pairingState.type ||
          newState.pin != _pairingState.pin) {
        _pairingState = newState;
        notifyListeners();

        // Stop polling on terminal states
        if (newState.type == PairingStateType.complete ||
            newState.type == PairingStateType.failed) {
          _stopPolling();
          // Refresh capsules after completion
          if (newState.type == PairingStateType.complete) {
            refreshCapsules();
          }
        }
      }
    } catch (e) {
      debugPrint('PairingService._pollState error: $e');
    }
  }

  /// Start the inviter flow for a capsule.
  void startInvite(String capsuleId) {
    if (!_initialized) return;
    _pairingState = const PairingStateData(type: PairingStateType.idle);
    notifyListeners();
    final result = _bindings.startInvite(capsuleId);
    debugPrint('PairingService.startInvite: FFI returned $result');
    if (result != 0) {
      _error = 'BLE peripheral init failed (code: $result)';
      notifyListeners();
      return;
    }
    _startPolling();
  }

  /// Start the joiner flow.
  void startJoin(String pieceName) {
    if (!_initialized) return;
    _pairingState = const PairingStateData(type: PairingStateType.idle);
    notifyListeners();
    final result = _bindings.startJoin(pieceName);
    debugPrint('PairingService.startJoin: FFI returned $result');
    if (result != 0) {
      _error = 'BLE central init failed (code: $result)';
      notifyListeners();
      return;
    }
    _startPolling();
  }

  /// Inviter: confirm the PIN was displayed / proceed.
  void confirmPin() {
    if (!_initialized) return;
    _bindings.confirmPin();
  }

  /// Joiner: submit the PIN entered by the user.
  void submitPin(String pin) {
    if (!_initialized) return;
    _bindings.submitPin(pin);
  }

  /// Cancel in-progress pairing.
  void cancelPairing() {
    if (!_initialized) return;
    _bindings.cancel();
    _stopPolling();
    _pairingState = const PairingStateData(type: PairingStateType.idle);
    notifyListeners();
  }

  /// Add a simulated accessory to a capsule.
  String? addSimAccessory(String capsuleId, String name) {
    if (!_initialized) return null;
    try {
      final json = _bindings.addSimAccessory(capsuleId, name);
      final result = jsonDecode(json) as Map<String, dynamic>;
      if (result.containsKey('error')) {
        _error = result['error'] as String?;
        notifyListeners();
        return null;
      }
      refreshCapsules();
      return result['device_id'] as String?;
    } catch (e) {
      debugPrint('PairingService.addSimAccessory error: $e');
      return null;
    }
  }

  @override
  void dispose() {
    _stopPolling();
    if (_initialized) {
      _bindings.cleanup();
    }
    super.dispose();
  }
}
