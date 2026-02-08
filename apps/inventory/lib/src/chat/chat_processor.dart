import 'dart:convert';
import '../core/inventory_api.dart';

class ChatProcessor {
  final InventoryApi _inventoryApi;

  ChatProcessor(this._inventoryApi);

  Future<void> processCommand(String jsonCommand) async {
    try {
      final command = jsonDecode(jsonCommand) as Map<String, dynamic>;

      switch (command['command']) {
        case 'add':
        final args = command['arguments'] as Map<String, dynamic>?;
        if (args == null) {
          throw StateError('Missing "arguments" for add command. Command received: ${jsonEncode(command)}');
        }

        final description = args['description'] as String?;
        final category = args['category'] as String?;
        final location = args['location'] as String?;

        if (description == null || category == null || location == null) {
          throw StateError('Missing required arguments (description, category, location) for add command. Arguments received: ${jsonEncode(args)}');
        }

        final isContainer = args['isContainer'] as bool? ?? false;
        String? containerId;
        if (isContainer) {
          containerId = args['containerId'] as String?;
          if (containerId == null || containerId.isEmpty) {
            throw StateError('Missing or empty "containerId" argument is required when creating a container. Arguments received: ${jsonEncode(args)}');
          }
        }

        final dynamic tagsValue = args['tags'];
        List<String> tagsList = [];
        if (tagsValue is String) {
          tagsList = tagsValue.split(',').map((s) => s.trim()).where((s) => s.isNotEmpty).toList();
        } else if (tagsValue is List) {
          tagsList = tagsValue.cast<String>();
        }

        final conflict = _inventoryApi.findDescriptionConflict(description);
        if (conflict != null) {
          throw StateError(
            'Description "$description" conflicts with existing item "${conflict.description}" '
            '(substring match). Use a more specific description or edit the existing item first.');
        }

        await _inventoryApi.addItem(
          category: category,
          description: description,
          location: location,
          tags: tagsList,
          isContainer: isContainer,
          containerId: containerId,
        );
        break;
      case 'delete':
        final args = command['arguments'] as Map<String, dynamic>?;
        if (args == null) {
          throw StateError('Missing "arguments" for delete command. Command received: ${jsonEncode(command)}');
        }
        final searchStr = (args['search_str'] ?? args['searchStr']) as String?;
        if (searchStr == null) {
          throw StateError('Missing "search_str" or "searchStr" argument for delete command. Arguments received: ${jsonEncode(args)}');
        }
        await _inventoryApi.deleteItem(
          searchStr: searchStr,
        );
        break;
      case 'move':
        final args = command['arguments'] as Map<String, dynamic>?;
        if (args == null) {
          throw StateError('Missing "arguments" for move command. Command received: ${jsonEncode(command)}');
        }
        final searchStr = (args['search_str'] ?? args['searchStr']) as String?;
        final newLocation = (args['new_location'] ?? args['newLocation']) as String?;
        if (searchStr == null || newLocation == null) {
          throw StateError('Missing arguments for move command. Arguments received: ${jsonEncode(args)}');
        }
        await _inventoryApi.moveItem(
          searchStr: searchStr,
          newLocation: newLocation,
        );
        break;
      case 'put-in':
        final args = command['arguments'] as Map<String, dynamic>?;
        if (args == null) {
          throw StateError('Missing "arguments" for put-in command. Command received: ${jsonEncode(command)}');
        }
        final searchStr = (args['search_str'] ?? args['searchStr']) as String?;
        final containerId = (args['container_id'] ?? args['containerId']) as String?;
        if (searchStr == null || containerId == null) {
          throw StateError('Missing arguments for put-in command. Arguments received: ${jsonEncode(args)}');
        }
        await _inventoryApi.putInContainer(
          searchStr: searchStr,
          containerId: containerId,
        );
        break;
      case 'remove-from-container':
        final args = command['arguments'] as Map<String, dynamic>?;
        if (args == null) {
          throw StateError('Missing "arguments" for remove-from-container command. Command received: ${jsonEncode(command)}');
        }
        final searchStr = (args['search_str'] ?? args['searchStr']) as String?;
        if (searchStr == null) {
          throw StateError('Missing "search_str" or "searchStr" argument for remove-from-container command. Arguments received: ${jsonEncode(args)}');
        }
        await _inventoryApi.removeFromContainer(
          searchStr: searchStr,
        );
        break;
      case 'edit-description':
        final args = command['arguments'] as Map<String, dynamic>?;
        if (args == null) {
          throw StateError('Missing "arguments" for edit-description command. Command received: ${jsonEncode(command)}');
        }
        final searchStr = (args['search_str'] ?? args['searchStr']) as String?;
        final newDescription = (args['new_description'] ?? args['newDescription']) as String?;
        if (searchStr == null || newDescription == null) {
          throw StateError('Missing arguments for edit-description command. Arguments received: ${jsonEncode(args)}');
        }
        await _inventoryApi.editDescription(
          searchStr: searchStr,
          newDescription: newDescription,
        );
        break;
      case 'create-container':
        final args = command['arguments'] as Map<String, dynamic>?;
        if (args == null) {
          throw StateError('Missing "arguments" for create-container command. Command received: ${jsonEncode(command)}');
        }
        final containerId = (args['container_id'] ?? args['containerId']) as String?;
        final location = args['location'] as String?;
        if (containerId == null || location == null) {
          throw StateError('Missing required arguments (container_id, location) for create-container command. Arguments received: ${jsonEncode(args)}');
        }
        final description = args['description'] as String?;
        await _inventoryApi.createContainer(
          containerId: containerId,
          location: location,
          description: description,
        );
        break;
      case 'add-tag':
        final args = command['arguments'] as Map<String, dynamic>?;
        if (args == null) {
          throw StateError('Missing "arguments" for add-tag command. Command received: ${jsonEncode(command)}');
        }
        final searchStr = (args['search_str'] ?? args['searchStr']) as String?;
        final tag = args['tag'] as String?;
        if (searchStr == null || tag == null) {
          throw StateError('Missing required arguments (search_str, tag) for add-tag command. Arguments received: ${jsonEncode(args)}');
        }
        await _inventoryApi.addTag(
          searchStr: searchStr,
          tag: tag,
        );
        break;
      case 'remove-tag':
        final args = command['arguments'] as Map<String, dynamic>?;
        if (args == null) {
          throw StateError('Missing "arguments" for remove-tag command. Command received: ${jsonEncode(command)}');
        }
        final searchStr = (args['search_str'] ?? args['searchStr']) as String?;
        final tag = args['tag'] as String?;
        if (searchStr == null || tag == null) {
          throw StateError('Missing required arguments (search_str, tag) for remove-tag command. Arguments received: ${jsonEncode(args)}');
        }
        await _inventoryApi.removeTag(
          searchStr: searchStr,
          tag: tag,
        );
        break;
      case 'group-put-in':
        final args = command['arguments'] as Map<String, dynamic>?;
        if (args == null) {
          throw StateError('Missing "arguments" for group-put-in command. Command received: ${jsonEncode(command)}');
        }
        final tag = args['tag'] as String?;
        final containerId = (args['container_id'] ?? args['containerId']) as String?;
        if (tag == null || containerId == null) {
          throw StateError('Missing required arguments (tag, container_id) for group-put-in command. Arguments received: ${jsonEncode(args)}');
        }
        await _inventoryApi.groupPutIn(
          tag: tag,
          containerId: containerId,
        );
        break;
      case 'group-remove-tag':
        final args = command['arguments'] as Map<String, dynamic>?;
        if (args == null) {
          throw StateError('Missing "arguments" for group-remove-tag command. Command received: ${jsonEncode(command)}');
        }
        final tag = args['tag'] as String?;
        if (tag == null) {
          throw StateError('Missing required argument (tag) for group-remove-tag command. Arguments received: ${jsonEncode(args)}');
        }
        await _inventoryApi.groupRemoveTag(
          tag: tag,
        );
        break;
      default:
        throw UnimplementedError('Command not found: ${command['command']}');
      }
    } catch (e) {
      // Re-throw as a single-line error to avoid breaking markdown rendering.
      final errorMessage = e.toString().replaceAll('\n', ' ');
      throw StateError('Error processing command: $errorMessage');
    }
  }
}
