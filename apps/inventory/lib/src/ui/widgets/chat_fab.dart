import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../../models/app_state.dart';
import '../chat_interface.dart';

class ChatFab extends StatelessWidget {
  const ChatFab({super.key});

  @override
  Widget build(BuildContext context) {
    return Consumer<AppState>(
      builder: (context, appState, child) {
        return FloatingActionButton(
          onPressed: () {
            Navigator.of(context).push(
              MaterialPageRoute(
                builder: (context) => const ChatInterface(),
                fullscreenDialog: true,
              ),
            );
          },
          child: const Icon(Icons.chat),
        );
      },
    );
  }
}
