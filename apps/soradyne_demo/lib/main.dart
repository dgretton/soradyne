import 'dart:io';

import 'package:flutter/material.dart';
import 'package:permission_handler/permission_handler.dart';
import 'package:provider/provider.dart';
import 'screens/activity_selector_screen.dart';
import 'services/album_service.dart';
import 'services/pairing_service.dart';
import 'theme/app_theme.dart';

Future<void> _requestBlePermissions() async {
  if (!Platform.isAndroid) return;
  await [
    Permission.bluetoothScan,
    Permission.bluetoothConnect,
    Permission.bluetoothAdvertise,
  ].request();
}

void main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await _requestBlePermissions();
  runApp(const SoradyneApp());
}

class SoradyneApp extends StatelessWidget {
  const SoradyneApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MultiProvider(
      providers: [
        ChangeNotifierProvider(create: (_) => AlbumService()),
        ChangeNotifierProvider(create: (_) => PairingService()),
      ],
      child: MaterialApp(
        title: 'Soradyne',
        theme: AppTheme.lightTheme,
        darkTheme: AppTheme.darkTheme,
        themeMode: ThemeMode.system,
        home: const ActivitySelectorScreen(),
        debugShowCheckedModeBanner: false,
      ),
    );
  }
}
