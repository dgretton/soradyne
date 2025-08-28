import 'package:flutter/material.dart';
import 'package:giantt_core/giantt_core.dart';
import 'screens/home_screen.dart';
import 'screens/chart_view_screen.dart';
import 'screens/item_detail_screen.dart';
import 'screens/add_item_screen.dart';
import 'services/giantt_service.dart';
import 'theme/app_theme.dart';

void main() {
  runApp(const GianttApp());
}

class GianttApp extends StatelessWidget {
  const GianttApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Giantt',
      theme: AppTheme.lightTheme,
      darkTheme: AppTheme.darkTheme,
      themeMode: ThemeMode.system,
      home: const GianttHomePage(),
      routes: {
        '/chart': (context) => const ChartViewScreen(),
        '/add-item': (context) => const AddItemScreen(),
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
    );
  }
}

class GianttHomePage extends StatefulWidget {
  const GianttHomePage({super.key});

  @override
  State<GianttHomePage> createState() => _GianttHomePageState();
}

class _GianttHomePageState extends State<GianttHomePage> {
  final GianttService _gianttService = GianttService();
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
