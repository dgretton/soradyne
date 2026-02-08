import 'package:flutter/material.dart';
import 'package:ai_chat_flutter/ai_chat_flutter.dart';
import 'package:provider/provider.dart';
import 'src/core/file_manager.dart';
import 'src/core/inventory_api.dart';
import 'src/ui/inventory_list_page.dart';
import 'src/models/app_state.dart';
import 'src/ui/widgets/build_banner.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();

  final legacyPath = await getInventoryFilePath();
  final api = InventoryApi(operationLogPath: legacyPath);
  await api.initialize(legacyPath);

  final settings = await SettingsService.loadSettings();

  runApp(InventoryApp(api: api, settings: settings));
}

class InventoryApp extends StatelessWidget {
  final InventoryApi api;
  final LLMSettings settings;
  const InventoryApp({super.key, required this.api, required this.settings});

  @override
  Widget build(BuildContext context) {
    return MultiProvider(
      providers: [
        ChangeNotifierProvider(create: (context) => AppState()),
        ChangeNotifierProvider.value(value: settings),
        Provider<InventoryApi>.value(value: api),
      ],
      child: MaterialApp(
        title: 'Inventory',
        theme: _buildPortalTheme(),
        home: const BuildBanner(child: InventoryListPage()),
      ),
    );
  }

  ThemeData _buildPortalTheme() {
    const portalWhite = Color(0xFFFAFAFA);
    const portalOffWhite = Color(0xFFF5F5F5);
    const portalSlate = Color(0xFF37474F);
    const portalDarkMetal = Color(0xFF263238);
    const portalOrange = Color(0xFFFF6D00);
    const portalBlue = Color(0xFF40C4FF);

    return ThemeData(
      useMaterial3: true,
      colorScheme: ColorScheme.light(
        primary: portalSlate,
        secondary: portalOrange,
        tertiary: portalBlue,
        surface: portalWhite,
        surfaceContainerHighest: portalOffWhite,
        onSurface: portalDarkMetal,
        onPrimary: portalWhite,
        onSecondary: portalWhite,
      ),
      scaffoldBackgroundColor: portalWhite,
      appBarTheme: AppBarTheme(
        backgroundColor: portalSlate,
        foregroundColor: portalWhite,
        elevation: 2,
        shadowColor: portalDarkMetal.withValues(alpha: 0.3),
      ),
      cardTheme: CardThemeData(
        color: portalOffWhite,
        elevation: 1,
        shadowColor: portalDarkMetal.withValues(alpha: 0.2),
      ),
      elevatedButtonTheme: ElevatedButtonThemeData(
        style: ElevatedButton.styleFrom(
          backgroundColor: portalOrange,
          foregroundColor: portalWhite,
        ),
      ),
      floatingActionButtonTheme: FloatingActionButtonThemeData(
        backgroundColor: portalOrange,
        foregroundColor: portalWhite,
      ),
    );
  }
}
