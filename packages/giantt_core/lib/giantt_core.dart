/// Core giantt logic for task dependency management
library giantt_core;

// Models
export 'src/models/giantt_item.dart';
export 'src/models/relation.dart';
export 'src/models/time_constraint.dart';
export 'src/models/duration.dart';
export 'src/models/status.dart';
export 'src/models/priority.dart';
export 'src/models/chart.dart';
export 'src/models/log_entry.dart';
export 'src/models/graph_exceptions.dart';

// Parser
export 'src/parser/giantt_parser.dart';
export 'src/parser/parse_exceptions.dart';

// Graph
export 'src/graph/giantt_graph.dart';
export 'src/graph/cycle_detector.dart';

// Storage
export 'src/storage/file_repository.dart';
export 'src/storage/atomic_file_writer.dart';
export 'src/storage/backup_manager.dart';
export 'src/storage/file_header_generator.dart';
export 'src/storage/path_resolver.dart';

// Validation
export 'src/validation/graph_doctor.dart';
export 'src/validation/issue_types.dart';

// Logging
export 'src/logging/log_repository.dart';
export 'src/logging/log_collection.dart';

// Storage helpers and commands used by the Flutter app service layer
export 'src/storage/dual_file_manager.dart';
export 'src/commands/command_interface.dart';

// Query layer (LLM tool-use integration)
export 'src/query/giantt_query.dart';
export 'src/commands/summary_command.dart';
export 'src/commands/blocked_command.dart';
export 'src/commands/deps_command.dart';
export 'src/commands/load_command.dart';
export 'src/commands/list_command.dart';

// Flow-based CRDT storage (Soradyne-backed)
export 'src/storage/flow_repository.dart';
export 'src/operations/giantt_operations.dart';
export 'src/ffi/flow_client.dart';
export 'src/ffi/soradyne_ffi.dart';
