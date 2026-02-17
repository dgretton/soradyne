import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'screens/activity_selector_screen.dart';
import 'services/album_service.dart';
import 'services/pairing_service.dart';
import 'theme/app_theme.dart';

void main() {
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
