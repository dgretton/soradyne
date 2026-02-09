import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:ai_chat_flutter/ai_chat_flutter.dart';
import 'screens/home_screen.dart';
import 'screens/chart_view_screen.dart';
import 'screens/item_detail_screen.dart';
import 'screens/add_item_screen.dart';
import 'screens/settings_screen.dart';
import 'services/giantt_service.dart';
import 'models/app_state.dart';
import 'theme/app_theme.dart';
import 'widgets/chat_fab.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();

  final gianttService = GianttService();
  await gianttService.initialize();

  final settings = await SettingsService.loadSettings();

  runApp(GianttApp(gianttService: gianttService, settings: settings));
}

class GianttApp extends StatelessWidget {
  final GianttService gianttService;
  final LLMSettings settings;

  const GianttApp({
    super.key,
    required this.gianttService,
    required this.settings,
  });

  @override
  Widget build(BuildContext context) {
    return MultiProvider(
      providers: [
        ChangeNotifierProvider(create: (_) => GianttAppState()),
        ChangeNotifierProvider.value(value: settings),
        Provider<GianttService>.value(value: gianttService),
      ],
      child: MaterialApp(
        title: 'Giantt',
        theme: AppTheme.lightTheme,
        darkTheme: AppTheme.darkTheme,
        themeMode: ThemeMode.system,
        home: const GianttHomePage(),
        routes: {
          '/settings': (context) => const SettingsScreen(),
        },
        onGenerateRoute: (settings) {
          if (settings.name?.startsWith('/item/') == true) {
            final itemId = settings.name!.substring(6);
            return MaterialPageRoute(
              builder: (context) => ItemDetailScreen(itemId: itemId),
            );
          }
          return null;
        },
      ),
    );
  }
}

class GianttHomePage extends StatefulWidget {
  const GianttHomePage({super.key});

  @override
  State<GianttHomePage> createState() => _GianttHomePageState();
}

class _GianttHomePageState extends State<GianttHomePage> {
  int _selectedIndex = 0;

  static const List<Widget> _pages = <Widget>[
    HomeScreen(),
    ChartViewScreen(),
    AddItemScreen(),
  ];

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
        children: _pages,
      ),
      floatingActionButton: const ChatFab(),
      bottomNavigationBar: BottomNavigationBar(
        items: const <BottomNavigationBarItem>[
          BottomNavigationBarItem(
            icon: Icon(Icons.home),
            label: 'Home',
          ),
          BottomNavigationBarItem(
            icon: Icon(Icons.timeline),
            label: 'Charts',
          ),
          BottomNavigationBarItem(
            icon: Icon(Icons.add),
            label: 'Add Item',
          ),
        ],
        currentIndex: _selectedIndex,
        onTap: _onItemTapped,
      ),
    );
  }
}
