import 'dart:async';

typedef ActionHandler<T> = Future<T> Function(Map<String, dynamic> parameters);

class ActionRegistry {
  final Map<String, ActionHandler> _handlers = {};
  
  void registerAction<T>(String actionName, ActionHandler<T> handler) {
    _handlers[actionName] = handler;
  }
  
  Future<dynamic> executeAction(String actionName, Map<String, dynamic> parameters) async {
    final handler = _handlers[actionName];
    if (handler == null) {
      throw Exception('Unknown action: $actionName');
    }
    return await handler(parameters);
  }
  
  List<String> get availableActions => _handlers.keys.toList();
}
