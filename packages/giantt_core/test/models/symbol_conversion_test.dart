import 'package:test/test.dart';
import 'package:giantt_core/giantt_core.dart';

void main() {
  group('Symbol Conversion Tests', () {
    group('GianttStatus', () {
      test('should have correct symbols matching Python', () {
        expect(GianttStatus.notStarted.symbol, equals('○'));
        expect(GianttStatus.inProgress.symbol, equals('◑'));
        expect(GianttStatus.blocked.symbol, equals('⊘'));
        expect(GianttStatus.completed.symbol, equals('●'));
      });

      test('should convert from symbol correctly', () {
        expect(GianttStatus.fromSymbol('○'), equals(GianttStatus.notStarted));
        expect(GianttStatus.fromSymbol('◑'), equals(GianttStatus.inProgress));
        expect(GianttStatus.fromSymbol('⊘'), equals(GianttStatus.blocked));
        expect(GianttStatus.fromSymbol('●'), equals(GianttStatus.completed));
      });

      test('should convert from name correctly', () {
        expect(GianttStatus.fromName('NOT_STARTED'), equals(GianttStatus.notStarted));
        expect(GianttStatus.fromName('IN_PROGRESS'), equals(GianttStatus.inProgress));
        expect(GianttStatus.fromName('BLOCKED'), equals(GianttStatus.blocked));
        expect(GianttStatus.fromName('COMPLETED'), equals(GianttStatus.completed));
      });

      test('should throw on invalid symbol', () {
        expect(() => GianttStatus.fromSymbol('X'), throwsArgumentError);
      });

      test('should throw on invalid name', () {
        expect(() => GianttStatus.fromName('INVALID'), throwsArgumentError);
      });
    });

    group('GianttPriority', () {
      test('should have correct symbols matching Python', () {
        expect(GianttPriority.lowest.symbol, equals(',,,'));
        expect(GianttPriority.low.symbol, equals('...'));
        expect(GianttPriority.neutral.symbol, equals(''));
        expect(GianttPriority.unsure.symbol, equals('?'));
        expect(GianttPriority.medium.symbol, equals('!'));
        expect(GianttPriority.high.symbol, equals('!!'));
        expect(GianttPriority.critical.symbol, equals('!!!'));
      });

      test('should convert from symbol correctly', () {
        expect(GianttPriority.fromSymbol(',,,'), equals(GianttPriority.lowest));
        expect(GianttPriority.fromSymbol('...'), equals(GianttPriority.low));
        expect(GianttPriority.fromSymbol(''), equals(GianttPriority.neutral));
        expect(GianttPriority.fromSymbol('?'), equals(GianttPriority.unsure));
        expect(GianttPriority.fromSymbol('!'), equals(GianttPriority.medium));
        expect(GianttPriority.fromSymbol('!!'), equals(GianttPriority.high));
        expect(GianttPriority.fromSymbol('!!!'), equals(GianttPriority.critical));
      });

      test('should convert from name correctly', () {
        expect(GianttPriority.fromName('LOWEST'), equals(GianttPriority.lowest));
        expect(GianttPriority.fromName('LOW'), equals(GianttPriority.low));
        expect(GianttPriority.fromName('NEUTRAL'), equals(GianttPriority.neutral));
        expect(GianttPriority.fromName('UNSURE'), equals(GianttPriority.unsure));
        expect(GianttPriority.fromName('MEDIUM'), equals(GianttPriority.medium));
        expect(GianttPriority.fromName('HIGH'), equals(GianttPriority.high));
        expect(GianttPriority.fromName('CRITICAL'), equals(GianttPriority.critical));
      });
    });

    group('RelationType', () {
      test('should have correct symbols matching Python', () {
        expect(RelationType.requires.symbol, equals('⊢'));
        expect(RelationType.anyof.symbol, equals('⋲'));
        expect(RelationType.supercharges.symbol, equals('≫'));
        expect(RelationType.indicates.symbol, equals('∴'));
        expect(RelationType.together.symbol, equals('∪'));
        expect(RelationType.conflicts.symbol, equals('⊟'));
        expect(RelationType.blocks.symbol, equals('►'));
        expect(RelationType.sufficient.symbol, equals('≻'));
      });

      test('should convert from symbol correctly', () {
        expect(RelationType.fromSymbol('⊢'), equals(RelationType.requires));
        expect(RelationType.fromSymbol('⋲'), equals(RelationType.anyof));
        expect(RelationType.fromSymbol('≫'), equals(RelationType.supercharges));
        expect(RelationType.fromSymbol('∴'), equals(RelationType.indicates));
        expect(RelationType.fromSymbol('∪'), equals(RelationType.together));
        expect(RelationType.fromSymbol('⊟'), equals(RelationType.conflicts));
        expect(RelationType.fromSymbol('►'), equals(RelationType.blocks));
        expect(RelationType.fromSymbol('≻'), equals(RelationType.sufficient));
      });

      test('should convert from name correctly', () {
        expect(RelationType.fromName('REQUIRES'), equals(RelationType.requires));
        expect(RelationType.fromName('ANYOF'), equals(RelationType.anyof));
        expect(RelationType.fromName('SUPERCHARGES'), equals(RelationType.supercharges));
        expect(RelationType.fromName('INDICATES'), equals(RelationType.indicates));
        expect(RelationType.fromName('TOGETHER'), equals(RelationType.together));
        expect(RelationType.fromName('CONFLICTS'), equals(RelationType.conflicts));
        expect(RelationType.fromName('BLOCKS'), equals(RelationType.blocks));
        expect(RelationType.fromName('SUFFICIENT'), equals(RelationType.sufficient));
      });
    });

    group('Duration parsing', () {
      test('should parse simple durations', () {
        final duration = GianttDuration.parse('5d');
        expect(duration.parts.length, equals(1));
        expect(duration.parts[0].amount, equals(5.0));
        expect(duration.parts[0].unit, equals('d'));
      });

      test('should parse compound durations', () {
        final duration = GianttDuration.parse('6mo8d3.5s');
        expect(duration.parts.length, equals(3));
        expect(duration.parts[0].amount, equals(6.0));
        expect(duration.parts[0].unit, equals('mo'));
        expect(duration.parts[1].amount, equals(8.0));
        expect(duration.parts[1].unit, equals('d'));
        expect(duration.parts[2].amount, equals(3.5));
        expect(duration.parts[2].unit, equals('s'));
      });

      test('should normalize units correctly', () {
        final hourDuration = DurationPart.create(2.0, 'hr');
        expect(hourDuration.unit, equals('h'));
        
        final dayDuration = DurationPart.create(1.0, 'day');
        expect(dayDuration.unit, equals('d'));
      });

      test('should calculate total seconds correctly', () {
        final duration = GianttDuration.parse('1h30min');
        expect(duration.totalSeconds, equals(5400.0)); // 3600 + 1800
      });
    });

    group('TimeConstraint parsing', () {
      test('should parse window constraints', () {
        final constraint = TimeConstraint.fromString('window(5d:2d,severe)');
        expect(constraint, isNotNull);
        expect(constraint!.type, equals(TimeConstraintType.window));
        expect(constraint.duration.toString(), equals('5d'));
        expect(constraint.gracePeriod?.toString(), equals('2d'));
        expect(constraint.consequenceType, equals(ConsequenceType.severe));
      });

      test('should parse deadline constraints', () {
        final constraint = TimeConstraint.fromString('due(2024-12-31:2d,severe)');
        expect(constraint, isNotNull);
        expect(constraint!.type, equals(TimeConstraintType.deadline));
        expect(constraint.dueDate, equals('2024-12-31'));
        expect(constraint.gracePeriod?.toString(), equals('2d'));
        expect(constraint.consequenceType, equals(ConsequenceType.severe));
      });

      test('should parse recurring constraints', () {
        final constraint = TimeConstraint.fromString('every(7d:1d,warn,stack)');
        expect(constraint, isNotNull);
        expect(constraint!.type, equals(TimeConstraintType.recurring));
        expect(constraint.interval?.toString(), equals('7d'));
        expect(constraint.gracePeriod?.toString(), equals('1d'));
        expect(constraint.consequenceType, equals(ConsequenceType.warning));
        expect(constraint.stack, isTrue);
      });
    });
  });
}
