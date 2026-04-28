import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../services/pairing_service.dart';

/// Single screen with state-driven content for the pairing flow.
///
/// Works for both inviter and joiner roles — the displayed UI changes
/// based on the `PairingStateData` from the pairing engine.
class PairingScreen extends StatefulWidget {
  final String? capsuleId;
  final bool isInviter;

  const PairingScreen({
    super.key,
    this.capsuleId,
    this.isInviter = true,
  });

  @override
  State<PairingScreen> createState() => _PairingScreenState();
}

class _PairingScreenState extends State<PairingScreen> {
  final _pinController = TextEditingController();

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      final service = context.read<PairingService>();
      if (widget.isInviter && widget.capsuleId != null) {
        service.startInvite(widget.capsuleId!);
      } else {
        service.startJoin('This device');
      }
    });
  }

  @override
  void dispose() {
    _pinController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(widget.isInviter ? 'Invite Device' : 'Join Capsule'),
        actions: [
          Consumer<PairingService>(
            builder: (context, service, _) {
              final state = service.pairingState;
              if (state.type != PairingStateType.complete &&
                  state.type != PairingStateType.failed) {
                return IconButton(
                  icon: const Icon(Icons.close),
                  onPressed: () {
                    service.cancelPairing();
                    Navigator.pop(context);
                  },
                );
              }
              return const SizedBox.shrink();
            },
          ),
        ],
      ),
      body: Consumer<PairingService>(
        builder: (context, service, _) {
          return _buildStateContent(context, service);
        },
      ),
    );
  }

  Widget _buildStateContent(BuildContext context, PairingService service) {
    final state = service.pairingState;

    switch (state.type) {
      case PairingStateType.idle:
        return _buildIdleState();
      case PairingStateType.awaitingVerification:
        return widget.isInviter
            ? _buildInviterVerification(state, service)
            : _buildJoinerVerification(state, service);
      case PairingStateType.transferring:
        return _buildTransferringState();
      case PairingStateType.complete:
        return _buildCompleteState(state);
      case PairingStateType.failed:
        return _buildFailedState(state, service);
    }
  }

  Widget _buildIdleState() {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          SizedBox(
            width: 80,
            height: 80,
            child: CircularProgressIndicator(
              strokeWidth: 3,
              color: Theme.of(context).colorScheme.primary,
            ),
          ),
          const SizedBox(height: 24),
          Text(
            widget.isInviter
                ? 'Waiting for device...'
                : 'Scanning for invitation...',
            style: const TextStyle(fontSize: 18),
          ),
          const SizedBox(height: 8),
          Text(
            widget.isInviter
                ? 'The other device should be in "Join" mode'
                : 'Looking for a device broadcasting an invitation',
            style: TextStyle(color: Colors.grey[600]),
            textAlign: TextAlign.center,
          ),
        ],
      ),
    );
  }

  Widget _buildInviterVerification(
      PairingStateData state, PairingService service) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              Icons.verified_user,
              size: 56,
              color: Theme.of(context).colorScheme.primary,
            ),
            const SizedBox(height: 24),
            const Text(
              'Verification PIN',
              style: TextStyle(fontSize: 16, fontWeight: FontWeight.w500),
            ),
            const SizedBox(height: 16),
            // Large PIN display
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 32, vertical: 16),
              decoration: BoxDecoration(
                color: Theme.of(context).colorScheme.surfaceContainerHighest,
                borderRadius: BorderRadius.circular(16),
              ),
              child: Text(
                state.pin ?? '------',
                style: TextStyle(
                  fontSize: 48,
                  fontWeight: FontWeight.bold,
                  letterSpacing: 12,
                  color: Theme.of(context).colorScheme.primary,
                ),
              ),
            ),
            const SizedBox(height: 24),
            Text(
              'Enter this PIN on the other device',
              style: TextStyle(color: Colors.grey[600]),
            ),
            const SizedBox(height: 32),
            FilledButton.icon(
              onPressed: () => service.confirmPin(),
              icon: const Icon(Icons.check),
              label: const Text('PIN Displayed — Continue'),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildJoinerVerification(
      PairingStateData state, PairingService service) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              Icons.pin,
              size: 56,
              color: Theme.of(context).colorScheme.primary,
            ),
            const SizedBox(height: 24),
            const Text(
              'Enter the PIN shown on the other device',
              style: TextStyle(fontSize: 16, fontWeight: FontWeight.w500),
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 24),
            SizedBox(
              width: 240,
              child: TextField(
                controller: _pinController,
                keyboardType: TextInputType.number,
                maxLength: 6,
                textAlign: TextAlign.center,
                style: const TextStyle(
                  fontSize: 32,
                  letterSpacing: 8,
                  fontWeight: FontWeight.bold,
                ),
                decoration: const InputDecoration(
                  counterText: '',
                  hintText: '000000',
                  border: OutlineInputBorder(),
                ),
              ),
            ),
            const SizedBox(height: 24),
            FilledButton.icon(
              onPressed: () {
                final pin = _pinController.text.trim();
                if (pin.length == 6) {
                  service.submitPin(pin);
                }
              },
              icon: const Icon(Icons.send),
              label: const Text('Submit PIN'),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildTransferringState() {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          SizedBox(
            width: 80,
            height: 80,
            child: CircularProgressIndicator(
              strokeWidth: 3,
              color: Theme.of(context).colorScheme.primary,
            ),
          ),
          const SizedBox(height: 24),
          const Text(
            'Exchanging keys...',
            style: TextStyle(fontSize: 18),
          ),
          const SizedBox(height: 8),
          Text(
            'Securely transferring capsule key material',
            style: TextStyle(color: Colors.grey[600]),
          ),
        ],
      ),
    );
  }

  Widget _buildCompleteState(PairingStateData state) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Icon(
              Icons.check_circle,
              size: 80,
              color: Colors.green,
            ),
            const SizedBox(height: 24),
            const Text(
              'Pairing Complete!',
              style: TextStyle(
                fontSize: 24,
                fontWeight: FontWeight.bold,
              ),
            ),
            const SizedBox(height: 16),
            if (state.capsuleId != null)
              Text(
                'Capsule: ${state.capsuleId!.substring(0, 8)}...',
                style: TextStyle(color: Colors.grey[600]),
              ),
            const SizedBox(height: 32),
            FilledButton(
              onPressed: () => Navigator.pop(context),
              child: const Text('Done'),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildFailedState(PairingStateData state, PairingService service) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Icon(
              Icons.error_outline,
              size: 80,
              color: Colors.red,
            ),
            const SizedBox(height: 24),
            const Text(
              'Pairing Failed',
              style: TextStyle(
                fontSize: 24,
                fontWeight: FontWeight.bold,
              ),
            ),
            const SizedBox(height: 12),
            Text(
              state.reason ?? 'Unknown error',
              style: TextStyle(color: Colors.grey[600]),
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 32),
            Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                OutlinedButton(
                  onPressed: () => Navigator.pop(context),
                  child: const Text('Back'),
                ),
                const SizedBox(width: 16),
                FilledButton.icon(
                  onPressed: () {
                    if (widget.isInviter && widget.capsuleId != null) {
                      service.startInvite(widget.capsuleId!);
                    } else {
                      service.startJoin('This device');
                    }
                  },
                  icon: const Icon(Icons.refresh),
                  label: const Text('Try Again'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
