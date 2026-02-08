import 'dart:io';
import 'package:flutter/services.dart' show rootBundle;
import 'package:path_provider/path_provider.dart';

Future<String> getInventoryFilePath() async {
  final directory = await getApplicationDocumentsDirectory();
  final path = '${directory.path}/inventory.txt';
  final file = File(path);

  if (!await file.exists()) {
    print('[FileManager] Inventory file not found in documents directory. Copying from assets...');
    try {
      final data = await rootBundle.loadString('inventory.txt');
      await file.writeAsString(data);
      print('[FileManager] Copied inventory.txt to $path');
    } catch (e) {
      print('[FileManager] Error copying asset: $e');
      // Create an empty file if asset loading fails, so the app can still run.
      await file.create();
    }
  } else {
    print('[FileManager] Found inventory file at $path');
  }
  return path;
}
