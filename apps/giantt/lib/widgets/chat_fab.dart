import 'package:flutter/material.dart';
import '../screens/chat_screen.dart';

class ChatFab extends StatelessWidget {
  const ChatFab({super.key});

  @override
  Widget build(BuildContext context) {
    return FloatingActionButton(
      onPressed: () {
        Navigator.of(context).push(
          MaterialPageRoute(
            builder: (context) => const GianttChatScreen(),
            fullscreenDialog: true,
          ),
        );
      },
      child: const Icon(Icons.chat),
    );
  }
}
