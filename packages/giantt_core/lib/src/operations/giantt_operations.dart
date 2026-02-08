/// Giantt operation types for Flow-based storage.
///
/// Maps CLI commands to convergent document operations:
/// - AddItem: Create a new task
/// - SetField: Modify a scalar field (title, status, priority, etc.)
/// - AddToSet: Add to a collection (tags, charts, relations, time constraints)
/// - RemoveFromSet: Remove from a collection (requires observed_add_ids)
/// - RemoveItem: Delete a task
library;

import '../models/giantt_item.dart';
import '../models/status.dart';
import '../models/priority.dart';
import '../models/duration.dart';
import '../models/time_constraint.dart';

/// A Giantt operation that can be converted to convergent document operations.
///
/// GianttOp provides a higher-level API for creating operations that map
/// to the underlying convergent document primitives.
abstract class GianttOp {
  /// Convert this operation to a list of raw operation maps.
  ///
  /// Each map matches the Rust Operation enum format.
  List<Map<String, dynamic>> toOperations();

  // ============================================================================
  // Factory constructors for creating operations
  // ============================================================================

  /// Create an AddItem operation for a new task.
  factory GianttOp.addItem(String itemId) = AddItemOp;

  /// Create a SetField operation for a scalar field.
  factory GianttOp.setField(String itemId, String field, dynamic value) = SetFieldOp;

  /// Create an AddToSet operation for a collection field.
  factory GianttOp.addToSet(String itemId, String setName, String element) = AddToSetOp;

  /// Create a RemoveFromSet operation.
  ///
  /// Requires [observedAddIds] to implement informed-remove semantics.
  factory GianttOp.removeFromSet(
    String itemId,
    String setName,
    String element,
    List<String> observedAddIds,
  ) = RemoveFromSetOp;

  /// Create a RemoveItem operation.
  factory GianttOp.removeItem(String itemId) = RemoveItemOp;

  // ============================================================================
  // Convenience factories for common operations
  // ============================================================================

  /// Set the title of an item.
  static GianttOp setTitle(String itemId, String title) {
    return SetFieldOp(itemId, 'title', title);
  }

  /// Set the status of an item.
  static GianttOp setStatus(String itemId, GianttStatus status) {
    return SetFieldOp(itemId, 'status', status.name);
  }

  /// Set the priority of an item.
  static GianttOp setPriority(String itemId, GianttPriority priority) {
    return SetFieldOp(itemId, 'priority', priority.name);
  }

  /// Set the duration of an item.
  static GianttOp setDuration(String itemId, GianttDuration duration) {
    return SetFieldOp(itemId, 'duration', duration.toString());
  }

  /// Set the comment on an item.
  static GianttOp setComment(String itemId, String? comment) {
    return SetFieldOp(itemId, 'comment', comment ?? '');
  }

  /// Add a tag to an item.
  static GianttOp addTag(String itemId, String tag) {
    return AddToSetOp(itemId, 'tags', tag);
  }

  /// Add a chart to an item.
  static GianttOp addChart(String itemId, String chart) {
    return AddToSetOp(itemId, 'charts', chart);
  }

  /// Add a requires relation.
  static GianttOp addRequires(String itemId, String targetId) {
    return AddToSetOp(itemId, 'requires', targetId);
  }

  /// Add an anyof relation.
  static GianttOp addAnyof(String itemId, String targetId) {
    return AddToSetOp(itemId, 'anyof', targetId);
  }

  /// Add a blocks relation.
  static GianttOp addBlocks(String itemId, String targetId) {
    return AddToSetOp(itemId, 'blocks', targetId);
  }

  /// Add a time constraint.
  static GianttOp addTimeConstraint(String itemId, TimeConstraint constraint) {
    return AddToSetOp(itemId, 'timeConstraints', constraint.toString());
  }

  /// Convert a GianttItem to a list of operations.
  ///
  /// Used for importing legacy files.
  static List<GianttOp> fromItem(GianttItem item) {
    final ops = <GianttOp>[];

    // Add the item itself
    ops.add(AddItemOp(item.id));

    // Set fields
    ops.add(SetFieldOp(item.id, 'title', item.title));
    ops.add(SetFieldOp(item.id, 'status', item.status.name));
    ops.add(SetFieldOp(item.id, 'priority', item.priority.name));
    ops.add(SetFieldOp(item.id, 'duration', item.duration.toString()));

    if (item.userComment != null && item.userComment!.isNotEmpty) {
      ops.add(SetFieldOp(item.id, 'comment', item.userComment!));
    }

    // Add tags
    for (final tag in item.tags) {
      ops.add(AddToSetOp(item.id, 'tags', tag));
    }

    // Add charts
    for (final chart in item.charts) {
      ops.add(AddToSetOp(item.id, 'charts', chart));
    }

    // Add relations
    for (final entry in item.relations.entries) {
      final relationType = entry.key.toLowerCase();
      for (final targetId in entry.value) {
        ops.add(AddToSetOp(item.id, relationType, targetId));
      }
    }

    // Add time constraints
    for (final constraint in item.timeConstraints) {
      ops.add(AddToSetOp(item.id, 'timeConstraints', constraint.toString()));
    }

    return ops;
  }
}

/// Operation to add a new item.
class AddItemOp implements GianttOp {
  final String itemId;

  AddItemOp(this.itemId);

  @override
  List<Map<String, dynamic>> toOperations() {
    return [
      {
        'AddItem': {
          'item_id': itemId,
          'item_type': 'GianttItem',
        },
      },
    ];
  }
}

/// Operation to set a field on an item.
class SetFieldOp implements GianttOp {
  final String itemId;
  final String field;
  final dynamic value;

  SetFieldOp(this.itemId, this.field, this.value);

  @override
  List<Map<String, dynamic>> toOperations() {
    return [
      {
        'SetField': {
          'item_id': itemId,
          'field': field,
          'value': _encodeValue(value),
        },
      },
    ];
  }

  static Map<String, dynamic> _encodeValue(dynamic value) {
    if (value == null) {
      return {'Null': null};
    } else if (value is bool) {
      return {'Bool': value};
    } else if (value is int) {
      return {'Int': value};
    } else if (value is String) {
      return {'String': value};
    } else {
      return {'String': value.toString()};
    }
  }
}

/// Operation to add an element to a set.
class AddToSetOp implements GianttOp {
  final String itemId;
  final String setName;
  final String element;

  AddToSetOp(this.itemId, this.setName, this.element);

  @override
  List<Map<String, dynamic>> toOperations() {
    return [
      {
        'AddToSet': {
          'item_id': itemId,
          'set_name': setName,
          'element': {'String': element},
        },
      },
    ];
  }
}

/// Operation to remove an element from a set.
///
/// Requires observed_add_ids to implement informed-remove semantics.
class RemoveFromSetOp implements GianttOp {
  final String itemId;
  final String setName;
  final String element;
  final List<String> observedAddIds;

  RemoveFromSetOp(this.itemId, this.setName, this.element, this.observedAddIds);

  @override
  List<Map<String, dynamic>> toOperations() {
    return [
      {
        'RemoveFromSet': {
          'item_id': itemId,
          'set_name': setName,
          'element': {'String': element},
          'observed_add_ids': observedAddIds,
        },
      },
    ];
  }
}

/// Operation to remove an item.
class RemoveItemOp implements GianttOp {
  final String itemId;

  RemoveItemOp(this.itemId);

  @override
  List<Map<String, dynamic>> toOperations() {
    return [
      {
        'RemoveItem': {
          'item_id': itemId,
        },
      },
    ];
  }
}
