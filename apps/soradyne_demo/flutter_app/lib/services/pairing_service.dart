import 'dart:async';
import 'dart:convert';
import 'package:flutter/foundation.dart';
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

  PairingService() {
    _initializeBindings();
  }

  void _initializeBindings() {
    try {
      debugPrint('PairingService: creating bindings...');
      _bindings = PairingBindings();
      final result = _bindings.init();
      if (result == 0) {
        _initialized = true;
        debugPrint('PairingService: initialized successfully');
        refreshCapsules();
      } else {
        _error = 'Failed to initialize pairing bridge (code: $result)';
        debugPrint('PairingService: init failed: $_error');
      }
    } catch (e) {
      _error = 'FFI initialization error: $e';
      debugPrint('PairingService: $_error');
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
    _bindings.startInvite(capsuleId);
    _startPolling();
  }

  /// Start the joiner flow.
  void startJoin(String pieceName) {
    if (!_initialized) return;
    _pairingState = const PairingStateData(type: PairingStateType.idle);
    notifyListeners();
    _bindings.startJoin(pieceName);
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
