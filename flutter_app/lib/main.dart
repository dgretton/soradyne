import 'package:flutter/material.dart';
import 'package:webview_flutter/webview_flutter.dart';
import 'dart:io';
import 'dart:convert';
import 'package:http/http.dart' as http;
import 'package:path_provider/path_provider.dart';

void main() {
  runApp(const SoradyneApp());
}

class SoradyneApp extends StatelessWidget {
  const SoradyneApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Soradyne',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: Colors.deepPurple),
        useMaterial3: true,
      ),
      home: const MainScreen(),
    );
  }
}

class MainScreen extends StatefulWidget {
  const MainScreen({super.key});

  @override
  State<MainScreen> createState() => _MainScreenState();
}

class _MainScreenState extends State<MainScreen> {
  int _selectedIndex = 0;
  Process? _serverProcess;
  bool _serverRunning = false;
  String _serverUrl = 'http://localhost:3030';

  @override
  void initState() {
    super.initState();
    _startServer();
  }

  @override
  void dispose() {
    _stopServer();
    super.dispose();
  }

  Future<void> _startServer() async {
    try {
      // Get the app's documents directory
      final appDir = await getApplicationDocumentsDirectory();
      final serverDir = Directory('${appDir.path}/soradyne_server');
      
      if (!await serverDir.exists()) {
        await serverDir.create(recursive: true);
      }

      // Start the Rust web server
      // Note: In a real app, you'd embed the Rust binary or use FFI
      // For now, this assumes you have the server binary available
      _serverProcess = await Process.start(
        'cargo',
        ['run', '--bin', 'web_album_server'],
        workingDirectory: serverDir.path,
      );

      // Give the server time to start
      await Future.delayed(const Duration(seconds: 2));
      
      // Check if server is responding
      try {
        final response = await http.get(Uri.parse('$_serverUrl/api/albums'));
        if (response.statusCode == 200) {
          setState(() {
            _serverRunning = true;
          });
        }
      } catch (e) {
        print('Server not responding: $e');
      }
    } catch (e) {
      print('Failed to start server: $e');
    }
  }

  Future<void> _stopServer() async {
    _serverProcess?.kill();
    _serverProcess = null;
    setState(() {
      _serverRunning = false;
    });
  }

  void _onItemTapped(int index) {
    setState(() {
      _selectedIndex = index;
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: IndexedStack(
        index: _selectedIndex,
        children: [
          // Home tab
          const Center(
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Icon(Icons.home, size: 64, color: Colors.grey),
                SizedBox(height: 16),
                Text(
                  'Welcome to Soradyne',
                  style: TextStyle(fontSize: 24, fontWeight: FontWeight.bold),
                ),
                SizedBox(height: 8),
                Text(
                  'Your decentralized photo album system',
                  style: TextStyle(fontSize: 16, color: Colors.grey),
                ),
              ],
            ),
          ),
          
          // Albums tab
          _serverRunning 
            ? AlbumWebView(serverUrl: _serverUrl)
            : const Center(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    CircularProgressIndicator(),
                    SizedBox(height: 16),
                    Text('Starting album server...'),
                  ],
                ),
              ),
          
          // Settings tab
          SettingsScreen(
            serverRunning: _serverRunning,
            serverUrl: _serverUrl,
            onRestartServer: () {
              _stopServer();
              _startServer();
            },
          ),
        ],
      ),
      bottomNavigationBar: BottomNavigationBar(
        items: const <BottomNavigationBarItem>[
          BottomNavigationBarItem(
            icon: Icon(Icons.home),
            label: 'Home',
          ),
          BottomNavigationBarItem(
            icon: Icon(Icons.photo_album),
            label: 'Albums',
          ),
          BottomNavigationBarItem(
            icon: Icon(Icons.settings),
            label: 'Settings',
          ),
        ],
        currentIndex: _selectedIndex,
        onTap: _onItemTapped,
      ),
    );
  }
}

class AlbumWebView extends StatefulWidget {
  final String serverUrl;

  const AlbumWebView({super.key, required this.serverUrl});

  @override
  State<AlbumWebView> createState() => _AlbumWebViewState();
}

class _AlbumWebViewState extends State<AlbumWebView> {
  late final WebViewController _controller;

  @override
  void initState() {
    super.initState();
    
    _controller = WebViewController()
      ..setJavaScriptMode(JavaScriptMode.unrestricted)
      ..setNavigationDelegate(
        NavigationDelegate(
          onProgress: (int progress) {
            // Update loading bar if needed
          },
          onPageStarted: (String url) {},
          onPageFinished: (String url) {},
          onWebResourceError: (WebResourceError error) {
            print('WebView error: ${error.description}');
          },
        ),
      )
      ..loadRequest(Uri.parse(widget.serverUrl));
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Photo Albums'),
        backgroundColor: Theme.of(context).colorScheme.inversePrimary,
        actions: [
          IconButton(
            icon: const Icon(Icons.refresh),
            onPressed: () {
              _controller.reload();
            },
          ),
        ],
      ),
      body: WebViewWidget(controller: _controller),
    );
  }
}

class SettingsScreen extends StatelessWidget {
  final bool serverRunning;
  final String serverUrl;
  final VoidCallback onRestartServer;

  const SettingsScreen({
    super.key,
    required this.serverRunning,
    required this.serverUrl,
    required this.onRestartServer,
  });

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Settings'),
        backgroundColor: Theme.of(context).colorScheme.inversePrimary,
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Card(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const Text(
                    'Server Status',
                    style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold),
                  ),
                  const SizedBox(height: 8),
                  Row(
                    children: [
                      Icon(
                        serverRunning ? Icons.check_circle : Icons.error,
                        color: serverRunning ? Colors.green : Colors.red,
                      ),
                      const SizedBox(width: 8),
                      Text(serverRunning ? 'Running' : 'Stopped'),
                    ],
                  ),
                  const SizedBox(height: 8),
                  Text('URL: $serverUrl'),
                  const SizedBox(height: 16),
                  ElevatedButton(
                    onPressed: onRestartServer,
                    child: const Text('Restart Server'),
                  ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 16),
          Card(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const Text(
                    'About Soradyne',
                    style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold),
                  ),
                  const SizedBox(height: 8),
                  const Text(
                    'Soradyne is a decentralized photo album system using '
                    'block storage and CRDT synchronization for collaborative '
                    'media management.',
                  ),
                  const SizedBox(height: 16),
                  const Text(
                    'Features:',
                    style: TextStyle(fontWeight: FontWeight.bold),
                  ),
                  const SizedBox(height: 4),
                  const Text('• Fault-tolerant block storage'),
                  const Text('• Collaborative editing with CRDTs'),
                  const Text('• Multi-resolution image rendering'),
                  const Text('• Offline-first design'),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}
