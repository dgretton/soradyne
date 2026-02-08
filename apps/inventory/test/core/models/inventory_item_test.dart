import 'package:flutter_test/flutter_test.dart';
import 'package:inventory/src/core/models/inventory_item.dart';

void main() {
  group('InventoryItem', () {
    test('supports value equality', () {
      expect(
        const InventoryItem(
          category: 'Tools',
          tags: ['heavy'],
          description: 'A hammer',
          location: 'Toolbox',
        ),
        const InventoryItem(
          category: 'Tools',
          tags: ['heavy'],
          description: 'A hammer',
          location: 'Toolbox',
        ),
      );
    });
  });
}
