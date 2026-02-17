import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../services/pairing_service.dart';
import 'pairing_screen.dart';

class CapsuleListScreen extends StatefulWidget {
  const CapsuleListScreen({super.key});

  @override
  State<CapsuleListScreen> createState() => _CapsuleListScreenState();
}

class _CapsuleListScreenState extends State<CapsuleListScreen> {
  @override
  void initState() {
    super.initState();
    // Refresh capsules when screen loads
    WidgetsBinding.instance.addPostFrameCallback((_) {
      context.read<PairingService>().refreshCapsules();
    });
  }

  void _showCreateCapsuleDialog() {
    final controller = TextEditingController();
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Create Capsule'),
        content: TextField(
          controller: controller,
          autofocus: true,
          decoration: const InputDecoration(
            labelText: 'Capsule name',
            hintText: "e.g. Dana's devices",
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () {
              final name = controller.text.trim();
              if (name.isNotEmpty) {
                context.read<PairingService>().createCapsule(name);
                Navigator.pop(ctx);
              }
            },
            child: const Text('Create'),
          ),
        ],
      ),
    );
  }

  void _showCapsuleDetail(CapsuleData capsule) {
    showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
      ),
      builder: (ctx) => _CapsuleDetailSheet(capsule: capsule),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Capsules'),
      ),
      body: Consumer<PairingService>(
        builder: (context, service, _) {
          if (!service.initialized) {
            return Center(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  const Icon(Icons.error_outline, size: 48, color: Colors.red),
                  const SizedBox(height: 16),
                  Text(
                    service.error ?? 'Pairing bridge not initialized',
                    textAlign: TextAlign.center,
                    style: TextStyle(color: Colors.grey[600]),
                  ),
                ],
              ),
            );
          }

          if (service.capsules.isEmpty) {
            return Center(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(Icons.devices_other, size: 64, color: Colors.grey[400]),
                  const SizedBox(height: 16),
                  Text(
                    'No capsules yet',
                    style: TextStyle(
                      fontSize: 18,
                      color: Colors.grey[600],
                    ),
                  ),
                  const SizedBox(height: 8),
                  Text(
                    'Create a capsule to group your devices',
                    style: TextStyle(color: Colors.grey[500]),
                  ),
                ],
              ),
            );
          }

          return ListView.builder(
            padding: const EdgeInsets.all(16),
            itemCount: service.capsules.length,
            itemBuilder: (context, index) {
              final capsule = service.capsules[index];
              return Card(
                margin: const EdgeInsets.only(bottom: 12),
                child: ListTile(
                  leading: CircleAvatar(
                    backgroundColor:
                        capsule.isActive ? Colors.green[100] : Colors.grey[200],
                    child: Icon(
                      Icons.security,
                      color: capsule.isActive ? Colors.green : Colors.grey,
                    ),
                  ),
                  title: Text(capsule.name),
                  subtitle: Text(
                    '${capsule.pieceCount} piece${capsule.pieceCount == 1 ? '' : 's'}',
                  ),
                  trailing: const Icon(Icons.chevron_right),
                  onTap: () => _showCapsuleDetail(capsule),
                ),
              );
            },
          );
        },
      ),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: _showCreateCapsuleDialog,
        icon: const Icon(Icons.add),
        label: const Text('Create Capsule'),
      ),
    );
  }
}

/// Bottom sheet showing capsule details and actions.
class _CapsuleDetailSheet extends StatelessWidget {
  final CapsuleData capsule;

  const _CapsuleDetailSheet({required this.capsule});

  @override
  Widget build(BuildContext context) {
    return DraggableScrollableSheet(
      initialChildSize: 0.6,
      minChildSize: 0.3,
      maxChildSize: 0.9,
      expand: false,
      builder: (context, scrollController) {
        return Padding(
          padding: const EdgeInsets.all(20),
          child: ListView(
            controller: scrollController,
            children: [
              // Handle
              Center(
                child: Container(
                  width: 40,
                  height: 4,
                  margin: const EdgeInsets.only(bottom: 20),
                  decoration: BoxDecoration(
                    color: Colors.grey[300],
                    borderRadius: BorderRadius.circular(2),
                  ),
                ),
              ),

              // Capsule name
              Text(
                capsule.name,
                style: const TextStyle(
                  fontSize: 24,
                  fontWeight: FontWeight.bold,
                ),
              ),
              const SizedBox(height: 4),
              Text(
                'ID: ${capsule.id.substring(0, 8)}...',
                style: TextStyle(color: Colors.grey[500], fontSize: 12),
              ),
              const SizedBox(height: 20),

              // Pieces section
              const Text(
                'Pieces',
                style: TextStyle(fontSize: 16, fontWeight: FontWeight.w600),
              ),
              const SizedBox(height: 8),
              if (capsule.pieces.isEmpty)
                Text(
                  'No pieces in this capsule',
                  style: TextStyle(color: Colors.grey[500]),
                )
              else
                ...capsule.pieces.map((piece) => Card(
                      child: ListTile(
                        leading: Icon(
                          piece.role == 'Accessory'
                              ? Icons.memory
                              : Icons.phone_android,
                          color: Theme.of(context).colorScheme.primary,
                        ),
                        title: Text(piece.name),
                        subtitle: Text(piece.role),
                      ),
                    )),

              const SizedBox(height: 24),

              // Actions
              const Text(
                'Actions',
                style: TextStyle(fontSize: 16, fontWeight: FontWeight.w600),
              ),
              const SizedBox(height: 8),

              FilledButton.icon(
                onPressed: () {
                  Navigator.pop(context);
                  Navigator.push(
                    context,
                    MaterialPageRoute(
                      builder: (_) => PairingScreen(
                        capsuleId: capsule.id,
                        isInviter: true,
                      ),
                    ),
                  );
                },
                icon: const Icon(Icons.person_add),
                label: const Text('Invite Device'),
              ),
              const SizedBox(height: 8),

              OutlinedButton.icon(
                onPressed: () {
                  final service = context.read<PairingService>();
                  service.addSimAccessory(capsule.id, 'Sim Accessory');
                  Navigator.pop(context);
                },
                icon: const Icon(Icons.memory),
                label: const Text('Add Sim Accessory'),
              ),
            ],
          ),
        );
      },
    );
  }
}
