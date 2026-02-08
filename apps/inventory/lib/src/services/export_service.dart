import 'dart:io';
import 'package:flutter/services.dart';
import 'package:path_provider/path_provider.dart';

class ExportService {
  /// Copies the provided text to the system clipboard
  static Future<void> copyToClipboard(String text) async {
    await Clipboard.setData(ClipboardData(text: text));
  }

  /// Saves the provided text to a file in the downloads directory
  static Future<String> saveToFile(String content, String filename) async {
    final directory = await getDownloadsDirectory() ?? await getApplicationDocumentsDirectory();
    final file = File('${directory.path}/$filename');
    await file.writeAsString(content);
    return file.path;
  }
}
