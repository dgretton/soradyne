import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../services/pairing_service.dart';
import 'album_list_screen.dart';
import 'capsule_list_screen.dart';
import 'flow_demo_screen.dart';

class ActivitySelectorScreen extends StatelessWidget {
  const ActivitySelectorScreen({super.key});

  void _openFlowDemo(BuildContext context) {
    final service = context.read<PairingService>();
    final capsules = service.capsules;

    if (capsules.isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text(
            'No capsules yet — pair with another device first.',
          ),
        ),
      );
      return;
    }

    if (capsules.length == 1) {
      Navigator.push(
        context,
        MaterialPageRoute(
          builder: (_) => FlowDemoScreen(
            capsuleId: capsules.first.id,
            capsuleName: capsules.first.name,
          ),
        ),
      );
      return;
    }

    // Multiple capsules — show a picker.
    showModalBottomSheet(
      context: context,
      builder: (ctx) => ListView(
        shrinkWrap: true,
        padding: const EdgeInsets.all(16),
        children: [
          const Padding(
            padding: EdgeInsets.only(bottom: 12),
            child: Text(
              'Choose a capsule',
              style: TextStyle(fontSize: 16, fontWeight: FontWeight.w600),
            ),
          ),
          ...capsules.map((c) => ListTile(
                leading: const Icon(Icons.security),
                title: Text(c.name),
                subtitle: Text('${c.pieceCount} piece${c.pieceCount == 1 ? '' : 's'}'),
                onTap: () {
                  Navigator.pop(ctx);
                  Navigator.push(
                    context,
                    MaterialPageRoute(
                      builder: (_) => FlowDemoScreen(
                        capsuleId: c.id,
                        capsuleName: c.name,
                      ),
                    ),
                  );
                },
              )),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Container(
        decoration: const BoxDecoration(
          gradient: LinearGradient(
            begin: Alignment.topLeft,
            end: Alignment.bottomRight,
            colors: [
              Color(0xFF667EEA),
              Color(0xFF764BA2),
            ],
          ),
        ),
        child: SafeArea(
          child: Padding(
            padding: const EdgeInsets.all(24.0),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const SizedBox(height: 40),
                const Text(
                  'Welcome to',
                  style: TextStyle(
                    color: Colors.white70,
                    fontSize: 24,
                    fontWeight: FontWeight.w300,
                  ),
                ),
                const Text(
                  'Soradyne',
                  style: TextStyle(
                    color: Colors.white,
                    fontSize: 48,
                    fontWeight: FontWeight.bold,
                  ),
                ),
                const SizedBox(height: 16),
                const Text(
                  'Choose your activity',
                  style: TextStyle(
                    color: Colors.white70,
                    fontSize: 18,
                  ),
                ),
                const SizedBox(height: 60),
                Expanded(
                  child: GridView.count(
                    crossAxisCount: 2,
                    crossAxisSpacing: 16,
                    mainAxisSpacing: 16,
                    children: [
                      _ActivityCard(
                        title: 'Photo Albums',
                        subtitle: 'Share & collaborate on media',
                        icon: Icons.photo_library_rounded,
                        onTap: () {
                          Navigator.push(
                            context,
                            MaterialPageRoute(
                              builder: (context) => const AlbumListScreen(),
                            ),
                          );
                        },
                      ),
                      _ActivityCard(
                        title: 'Flow Demo',
                        subtitle: 'Shared notes via CRDT sync',
                        icon: Icons.stream_rounded,
                        onTap: () => _openFlowDemo(context),
                      ),
                      _ActivityCard(
                        title: 'Network Demo',
                        subtitle: 'Peer discovery & sync',
                        icon: Icons.network_check_rounded,
                        onTap: () {
                          Navigator.push(
                            context,
                            MaterialPageRoute(
                              builder: (context) =>
                                  const CapsuleListScreen(),
                            ),
                          );
                        },
                      ),
                      _ActivityCard(
                        title: 'Storage Demo',
                        subtitle: 'Block storage system',
                        icon: Icons.storage_rounded,
                        onTap: () {
                          // TODO: Navigate to storage demo
                          ScaffoldMessenger.of(context).showSnackBar(
                            const SnackBar(
                              content: Text('Storage demo coming soon!'),
                            ),
                          );
                        },
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _ActivityCard extends StatelessWidget {
  final String title;
  final String subtitle;
  final IconData icon;
  final VoidCallback onTap;

  const _ActivityCard({
    required this.title,
    required this.subtitle,
    required this.icon,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return Card(
      elevation: 8,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(20),
      ),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(20),
        child: Padding(
          padding: const EdgeInsets.all(20),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(
                icon,
                size: 48,
                color: Theme.of(context).colorScheme.primary,
              ),
              const SizedBox(height: 16),
              Text(
                title,
                style: const TextStyle(
                  fontSize: 18,
                  fontWeight: FontWeight.bold,
                ),
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 8),
              Text(
                subtitle,
                style: TextStyle(
                  fontSize: 14,
                  color: Colors.grey[600],
                ),
                textAlign: TextAlign.center,
              ),
            ],
          ),
        ),
      ),
    );
  }
}
