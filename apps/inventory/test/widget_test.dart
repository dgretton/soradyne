import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:inventory/src/core/inventory_api.dart';
import 'package:inventory/src/core/models/inventory_entry.dart';
import 'package:inventory/src/models/app_state.dart';
import 'package:inventory/src/ui/inventory_list_page.dart';
import 'package:provider/provider.dart';

class MockInventoryApi implements InventoryApi {
  List<InventoryEntry> mockItems = [];
  Object? errorToThrow;

  @override
  Future<List<InventoryEntry>> search(String searchStr) async {
    if (errorToThrow != null) {
      throw errorToThrow!;
    }
    return mockItems;
  }

  @override
  Future<void> initialize(String legacyFilePath) async {
    // Do nothing, this is a mock.
  }

  // We don't need to implement the other methods for these widget tests.
  // They will throw UnimplementedError if called, which is fine.
  @override
  String get operationLogPath => '';
  @override
  Future<void> addItem({required String category, required String description, required String location, List<String> tags = const [], bool isContainer = false, String? containerId}) => throw UnimplementedError();
  @override
  Future<bool> containerExists(String containerId) => throw UnimplementedError();
  @override
  Future<void> deleteItem({required String searchStr}) => throw UnimplementedError();
  @override
  Future<void> editDescription({required String searchStr, required String newDescription}) => throw UnimplementedError();
  @override
  Future<InventoryEntry> findUniqueEntry(String searchStr) => throw UnimplementedError();
  @override
  Future<void> moveItem({required String searchStr, required String newLocation}) => throw UnimplementedError();
  @override
  Future<void> putInContainer({required String searchStr, required String containerId}) => throw UnimplementedError();
  @override
  Future<void> removeFromContainer({required String searchStr}) => throw UnimplementedError();
  @override
  Future<void> createContainer({required String containerId, required String location, String? description}) => throw UnimplementedError();
  @override
  Future<void> addTag({required String searchStr, required String tag}) => throw UnimplementedError();
  @override
  Future<void> removeTag({required String searchStr, required String tag}) => throw UnimplementedError();
  @override
  Future<void> groupPutIn({required String tag, required String containerId}) => throw UnimplementedError();
  @override
  Future<void> groupRemoveTag({required String tag}) => throw UnimplementedError();
  @override
  String exportToLegacyFormat() => throw UnimplementedError();
  @override
  bool descriptionIsUnique(String description) => throw UnimplementedError();
  @override
  InventoryEntry? findDescriptionConflict(String description) => throw UnimplementedError();
  @override
  InventoryEntry findByIdPrefix(String idPrefix) => throw UnimplementedError();
}

void main() {
  late MockInventoryApi mockApi;

  setUp(() {
    mockApi = MockInventoryApi();
  });

  testWidgets('InventoryListPage shows empty message when no items exist',
      (WidgetTester tester) async {
    // Arrange: mock API returns an empty list
    mockApi.mockItems = [];

    await tester.pumpWidget(
      MultiProvider(
        providers: [
          Provider<InventoryApi>.value(value: mockApi),
          ChangeNotifierProvider(create: (_) => AppState()),
        ],
        child: const MaterialApp(
          home: InventoryListPage(),
        ),
      ),
    );

    // Act: Wait for the UI to settle
    await tester.pumpAndSettle();

    // Assert
    expect(find.text('No inventory items found.'), findsOneWidget);
    expect(find.byType(CircularProgressIndicator), findsNothing);
  });

  testWidgets('InventoryListPage shows items from inventory file',
      (WidgetTester tester) async {
    // Arrange: mock API returns a list of items
    mockApi.mockItems = [
      const InventoryEntry(id: '1', category: 'Tools', description: 'Hammer', location: 'Toolbox'),
      const InventoryEntry(id: '2', category: 'Decor', description: 'Vase', location: 'Shelf'),
    ];

    await tester.pumpWidget(
      MultiProvider(
        providers: [
          Provider<InventoryApi>.value(value: mockApi),
          ChangeNotifierProvider(create: (_) => AppState()),
        ],
        child: const MaterialApp(
          home: InventoryListPage(),
        ),
      ),
    );

    // Act: Wait for the UI to settle
    await tester.pumpAndSettle();

    // Assert
    expect(find.text('Hammer'), findsOneWidget);
    expect(find.text('Toolbox'), findsOneWidget);
    expect(find.text('Vase'), findsOneWidget);
    expect(find.text('Shelf'), findsOneWidget);
    expect(find.byType(CircularProgressIndicator), findsNothing);
    expect(find.byType(ListTile), findsNWidgets(2));
  });

  testWidgets('InventoryListPage handles errors gracefully',
      (WidgetTester tester) async {
    // Arrange: mock API throws an error
    mockApi.errorToThrow = Exception('Failed to load');

    await tester.pumpWidget(
      MultiProvider(
        providers: [
          Provider<InventoryApi>.value(value: mockApi),
          ChangeNotifierProvider(create: (_) => AppState()),
        ],
        child: const MaterialApp(
          home: InventoryListPage(),
        ),
      ),
    );

    // Act: Wait for the UI to settle
    await tester.pumpAndSettle();

    // Assert
    expect(find.text('Error: Exception: Failed to load'), findsOneWidget);
    expect(find.byType(CircularProgressIndicator), findsNothing);
  });
}
