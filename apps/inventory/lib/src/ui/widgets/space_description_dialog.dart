import 'dart:async';
import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';

class SpaceDescriptionDialog extends StatefulWidget {
  final String initialDescription;

  const SpaceDescriptionDialog({
    super.key,
    required this.initialDescription,
  });

  @override
  State<SpaceDescriptionDialog> createState() => _SpaceDescriptionDialogState();
}

class _SpaceDescriptionDialogState extends State<SpaceDescriptionDialog> {
  late TextEditingController _controller;
  Timer? _debounce;
  static const _draftKey = 'space_description_draft';

  @override
  void initState() {
    super.initState();
    _controller = TextEditingController(text: widget.initialDescription);
    _loadDraft();
    _controller.addListener(_onTextChanged);
  }

  @override
  void dispose() {
    _debounce?.cancel();
    _controller.removeListener(_onTextChanged);
    _controller.dispose();
    super.dispose();
  }

  void _onTextChanged() {
    if (_debounce?.isActive ?? false) _debounce!.cancel();
    _debounce = Timer(const Duration(milliseconds: 500), () async {
      final prefs = await SharedPreferences.getInstance();
      await prefs.setString(_draftKey, _controller.text);
    });
  }

  Future<void> _loadDraft() async {
    final prefs = await SharedPreferences.getInstance();
    final draft = prefs.getString(_draftKey);
    // Only load draft if the initial description is empty, to avoid overwriting good data
    if (draft != null && widget.initialDescription.isEmpty) {
      _controller.text = draft;
    }
  }

  Future<void> _clearDraft() async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.remove(_draftKey);
  }

  @override
  Widget build(BuildContext context) {
    return Dialog(
      child: Container(
        width: MediaQuery.of(context).size.width * 0.9,
        height: MediaQuery.of(context).size.height * 0.8,
        padding: const EdgeInsets.all(24),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(
                  Icons.home,
                  color: Theme.of(context).colorScheme.primary,
                ),
                const SizedBox(width: 8),
                Text(
                  'Living Space Description',
                  style: Theme.of(context).textTheme.headlineSmall,
                ),
                const Spacer(),
                IconButton(
                  icon: const Icon(Icons.close),
                  onPressed: () => Navigator.of(context).pop(),
                ),
              ],
            ),
            const SizedBox(height: 16),
            Text(
              'Describe your living spaces in detail. This helps the AI understand where items can be stored and provides better suggestions.',
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                color: Theme.of(context).colorScheme.onSurface.withValues(alpha: 0.7),
              ),
            ),
            const SizedBox(height: 16),
            Expanded(
              child: TextField(
                controller: _controller,
                maxLines: null,
                expands: true,
                textAlignVertical: TextAlignVertical.top,
                decoration: const InputDecoration(
                  hintText: 'Example:\n\nMy apartment has a bedroom with a large closet on the right side. The closet has three sections: left side for hanging clothes, center with shelves for folded items, and right side with shoe storage...\n\nThe living room has a tall bookshelf unit near the window with 7 shelves. The kitchen has upper and lower cabinets, with the upper cabinets used for dishes and the lower ones for pots and cleaning supplies...',
                  border: OutlineInputBorder(),
                  alignLabelWithHint: true,
                ),
              ),
            ),
            const SizedBox(height: 16),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                TextButton(
                  onPressed: () => Navigator.of(context).pop(),
                  child: const Text('Cancel'),
                ),
                const SizedBox(width: 8),
                ElevatedButton(
                  onPressed: () {
                    _clearDraft();
                    Navigator.of(context).pop(_controller.text);
                  },
                  child: const Text('Save'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
