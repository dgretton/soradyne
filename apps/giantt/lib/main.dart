import 'package:flutter/material.dart';
import 'package:giantt_core/giantt_core.dart';

void main() {
  runApp(const GianttApp());
}

class GianttApp extends StatelessWidget {
  const GianttApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Giantt',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: Colors.deepPurple),
        useMaterial3: true,
      ),
      home: const GianttHomePage(title: 'Giantt - Task Dependencies'),
    );
  }
}

class GianttHomePage extends StatefulWidget {
  const GianttHomePage({super.key, required this.title});

  final String title;

  @override
  State<GianttHomePage> createState() => _GianttHomePageState();
}

class _GianttHomePageState extends State<GianttHomePage> {
  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        backgroundColor: Theme.of(context).colorScheme.inversePrimary,
        title: Text(widget.title),
      ),
      body: const Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: <Widget>[
            Text(
              'Giantt Flutter App',
              style: TextStyle(fontSize: 24),
            ),
            SizedBox(height: 16),
            Text(
              'Task dependency management powered by Soradyne',
            ),
          ],
        ),
      ),
    );
  }
}
