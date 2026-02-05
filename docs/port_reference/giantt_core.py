from typing import List, Optional, Tuple, Dict, Set
from dataclasses import dataclass, field
import re
from enum import Enum
import json
from datetime import datetime, timezone
from pathlib import Path

class Status(Enum):
    NOT_STARTED = "○"
    IN_PROGRESS = "◑"
    BLOCKED = "⊘"
    COMPLETED = "●"

class Priority(Enum):
    LOWEST = ",,,"
    LOW = "..."
    NEUTRAL = ""
    UNSURE = "?"
    MEDIUM = "!"
    HIGH = "!!"
    CRITICAL = "!!!"

### Relations
# Each relation type uses a symbol followed by comma-separated target IDs:
# - `⊢` REQUIRES (left side tick)
# - `⋲` ANYOF (alternate paths)
# - `≫` SUPERCHARGES (much greater than - suggests enhancement)
# - `∴` INDICATES (therefore - suggests consequence)
# - `∪` TOGETHER (shows combination)
# - `⊟` CONFLICTS (suggests blocking)
# - `►` (redundancy symbol) BLOCKS (shows all items with ⊢ [REQUIRES] to this item)
# - `≻` (redundancy symbol) SUFFICIENT (shows all items with ⋲ [ANY] to this item)

class RelationType(Enum):
    REQUIRES = "⊢"
    ANYOF = "⋲"
    SUPERCHARGES = "≫"
    INDICATES = "∴"
    TOGETHER = "∪"
    CONFLICTS = "⊟"
    BLOCKS = "►"
    SUFFICIENT = "≻"

class TimeConstraintType(Enum):
    WINDOW = "window"
    DEADLINE = "deadline"
    RECURRING = "recurring"

class ConsequenceType(Enum):
    SEVERE = "severe"
    WARNING = "warn"
    ESCALATING = "escalating"

class EscalationRate(Enum):
    LOWEST = ",,,"
    LOW = "..."
    NEUTRAL = ""
    UNSURE = "?"
    MEDIUM = "!"
    HIGH = "!!"
    CRITICAL = "!!!"

class TimeConstraintType(Enum):
    WINDOW = "window"
    DEADLINE = "deadline"
    RECURRING = "recurring"

class ConsequenceType(Enum):
    SEVERE = "severe"
    WARNING = "warn"
    ESCALATING = "escalating"

class EscalationRate(Enum):
    LOWEST = ",,,"
    LOW = "..."
    NEUTRAL = ""
    UNSURE = "?"
    MEDIUM = "!"
    HIGH = "!!"
    CRITICAL = "!!!"


@dataclass(frozen=True)
class DurationPart:
    """Represents a single part of a duration with an amount and unit."""
    amount: float
    unit: str

    _UNIT_SECONDS = {
        's': 1,
        'min': 60,
        'h': 3600,
        'hr': 3600,
        'd': 86400,
        'w': 604800,
        'mo': 2592000,  # 30 days
        'y': 31536000,  # 365 days
    }

    _UNIT_NORMALIZE = {
        'hr': 'h',
        'minute': 'min',
        'minutes': 'min',
        'hour': 'h',
        'hours': 'h',
        'day': 'd',
        'days': 'd',
        'week': 'w',
        'weeks': 'w',
        'month': 'mo',
        'months': 'mo',
        'year': 'y',
        'years': 'y'
    }

    @classmethod
    def create(cls, amount: float, unit: str) -> 'DurationPart':
        """Factory method to create a normalized DurationPart."""
        normalized_unit = cls._UNIT_NORMALIZE.get(unit, unit)

        if normalized_unit not in cls._UNIT_SECONDS:
            raise ValueError(f"Invalid duration unit: {unit}")

        return cls(amount, normalized_unit)

    def __post_init__(self):
        """Validate the unit."""
        if self.unit not in self._UNIT_SECONDS:
            raise ValueError(f"Invalid duration unit: {self.unit}")

    @property
    def total_seconds(self) -> float:
        """Get total seconds."""
        return self.amount * self._UNIT_SECONDS[self.unit]

    def __str__(self):
        # For whole numbers, display as integers
        amount_str = str(int(self.amount)) if self.amount.is_integer() else f"{self.amount}"
        return f"{amount_str}{self.unit}"

    def __hash__(self):
        return hash((self.amount, self.unit))


@dataclass(frozen=True)
class Duration:
    """Handles compound durations like '6mo8d3.5s'."""

    parts: List[DurationPart] = field(default_factory=list)

    @classmethod
    def parse(cls, duration_str: str) -> 'Duration':
        """Parse a duration string into a Duration object."""
        if not duration_str:
            raise ValueError("Empty duration string")

        pattern = r'(\d+\.?\d*)([a-zA-Z]+)'
        matches = re.finditer(pattern, duration_str)
        parts = []

        for match in matches:
            amount = float(match.group(1))
            unit = match.group(2)
            parts.append(DurationPart.create(amount, unit))

        if not parts:
            raise ValueError(f"No valid duration parts found in: {duration_str}")

        return cls(parts)

    def total_seconds(self):
        """Get total duration in seconds."""
        return sum(part.total_seconds for part in self.parts)

    def __str__(self):
        """String representation of duration."""
        if not self.parts:
            return "0s"
        return "".join(str(part) for part in self.parts)

    def __add__(self, other):
        """Add two durations."""
        total_seconds = self.total_seconds() + other.total_seconds()

        # Convert back to largest sensible unit
        for unit, seconds in sorted(self._UNIT_SECONDS.items(), 
                                  key=lambda x: x[1], reverse=True):
            if total_seconds >= seconds:
                amount = total_seconds / seconds
                if amount.is_integer():
                    amount = int(amount)
                return Duration([DurationPart(amount, unit)])

        return Duration([DurationPart(total_seconds, 's')])

    def __eq__(self, other):
        """Compare two durations."""
        if not isinstance(other, Duration):
            return NotImplemented
        return self.total_seconds() == other.total_seconds()

    def __lt__(self, other):
        """Compare two durations."""
        if not isinstance(other, Duration):
            return NotImplemented
        return self.total_seconds() < other.total_seconds()

    def __gt__(self, other):
        """Compare two durations."""
        if not isinstance(other, Duration):
            return NotImplemented
        return self.total_seconds() > other.total_seconds()

    def __le__(self, other):
        """Compare two durations."""
        if not isinstance(other, Duration):
            return NotImplemented
        return self.total_seconds() <= other.total_seconds()

    def __ge__(self, other):
        """Compare two durations."""
        if not isinstance(other, Duration):
            return NotImplemented
        return self.total_seconds() >= other.total_seconds()

    def __hash__(self):
        return hash(tuple(self.parts))


@dataclass
class TimeWindow:
    """Represents a time window with an optional grace period."""
    window: DurationPart
    grace_period: Optional[DurationPart] = None

    @classmethod
    def parse(cls, window_str: str) -> 'TimeWindow':
        """Parse a time window string.

        Args:
            window_str: String like '5d' or '5d:2d' (with grace period)

        Returns:
            TimeWindow object
        """
        parts = window_str.split(':')
        window = DurationPart.parse(parts[0])
        grace_period = DurationPart.parse(parts[1]) if len(parts) > 1 else None
        return cls(window, grace_period)

    def __str__(self) -> str:
        """String representation of time window."""
        base = str(self.window)
        if self.grace_period:
            base += f":{self.grace_period}"
        return base


@dataclass
class TimeConstraint:
    type: TimeConstraintType
    duration: Duration
    grace_period: Optional[Duration] = None
    consequence_type: ConsequenceType = ConsequenceType.WARNING
    escalation_rate: EscalationRate = EscalationRate.NEUTRAL
    due_date: Optional[str] = None
    interval: Optional[Duration] = None
    stack: bool = False

    @classmethod
    def from_string(cls, constraint_str: str) -> Optional['TimeConstraint']:
        if not constraint_str:
            return None

        # Parse window constraints
        window_match = re.match(r'window\((\d+[smhdwy])(:\d+[smhdwy])?,([^)]+)\)', constraint_str)
        if window_match:
            window = Duration.parse(window_match.group(1))
            grace = Duration.parse(window_match.group(2)[1:]) if window_match.group(2) else None
            consequence = cls._parse_consequence(window_match.group(3))

            return cls(
                type=TimeConstraintType.WINDOW,
                duration=window,
                grace_period=grace,
                consequence_type=consequence['type'],
                escalation_rate=consequence['rate']
            )

        # Parse deadline constraints
        deadline_match = re.match(r'due\((\d{4}-\d{2}-\d{2})(:\d+[smhdwy])?,([^)]+)\)', constraint_str)
        if deadline_match:
            due_date = deadline_match.group(1)
            grace = Duration.parse(deadline_match.group(2)[1:]) if deadline_match.group(2) else None
            consequence = cls._parse_consequence(deadline_match.group(3))

            return cls(
                type=TimeConstraintType.DEADLINE,
                duration=Duration.parse('1d'), # Default to 1 day for deadline
                grace_period=grace,
                consequence_type=consequence['type'],
                escalation_rate=consequence['rate'],
                due_date=due_date
            )

        # Parse recurring constraints
        recurring_match = re.match(r'every\((\d+[smhdwy])(:\d+[smhdwy])?,([^)]+)\)', constraint_str)
        if recurring_match:
            interval = Duration.parse(recurring_match.group(1))
            grace = Duration.parse(recurring_match.group(2)[1:]) if recurring_match.group(2) else None
            consequence_str = recurring_match.group(3)

            stack = 'stack' in consequence_str
            consequence_str = consequence_str.replace(',stack', '')
            consequence = cls._parse_consequence(consequence_str)

            return cls(
                type=TimeConstraintType.RECURRING,
                duration=interval,
                grace_period=grace,
                consequence_type=consequence['type'],
                escalation_rate=consequence['rate'],
                interval=interval,
                stack=stack
            )

        raise ValueError(f"Invalid time constraint format: {constraint_str}")

    def __str__(self):
        base_str = {
            TimeConstraintType.WINDOW: f"window({self.duration}",
            TimeConstraintType.DEADLINE: f"due({self.due_date}",
            TimeConstraintType.RECURRING: f"every({self.interval}",
        }[self.type]

        if self.grace_period:
            base_str += f":{self.grace_period}"

        base_str += f",{self.consequence_type.value}"
        if self.escalation_rate != EscalationRate.NEUTRAL:
            base_str += f",escalate:{self.escalation_rate.value}"

        if self.type == TimeConstraintType.RECURRING and self.stack:
            base_str += ",stack"

        return base_str + ")"

    @staticmethod
    def _parse_consequence(consequence_str: str) -> dict:
        parts = consequence_str.split(',')
        base_consequence = parts[0].strip()

        if len(parts) > 1 and parts[1].startswith('escalate:'):
            rate_str = parts[1][9:]  # Remove 'escalate:'
            return {
                'type': ConsequenceType.ESCALATING,
                'rate': EscalationRate(rate_str) if rate_str else EscalationRate.NEUTRAL
            }

        return {
            'type': ConsequenceType(base_consequence),
            'rate': EscalationRate.NEUTRAL
        }


def parse_pre_title_section(pre_title: str) -> Tuple[str, str, str]:
    """Parse the pre-title section into status, id+priority, and duration."""
    # Updated pattern to be more flexible with whitespace
    pattern = r'^([○◑⊘●])\s+([^\s]+)\s+([^\s"]+)'
    match = re.match(pattern, pre_title)

    if not match:
        raise ValueError(f"Invalid pre-title format: {pre_title}")

    status = match.group(1)
    id_priority = match.group(2)
    duration = match.group(3).strip()

    return status, id_priority, duration

@dataclass
class GianttItem:
    id: str
    title: str = ""
    description: str = ""
    status: Status = Status.NOT_STARTED
    priority: Priority = Priority.NEUTRAL
    duration: Duration = Duration()
    charts: List[str] = field(default_factory=list)
    tags: List[str] = field(default_factory=list)
    relations: dict = field(default_factory=dict)
    time_constraint: Optional[TimeConstraint] = None
    user_comment: Optional[str] = None
    auto_comment: Optional[str] = None
    occlude: bool = False

    # type-check everything
    def __init__(self, id: str, title: str, description: str, status: Status, priority: Priority, duration: Duration, charts: List[str], tags: List[str], relations: dict, time_constraint: Optional[TimeConstraint], user_comment: Optional[str], auto_comment: Optional[str], occlude: bool = False):
        if not isinstance(id, str):
            raise TypeError(f"id must be a string, not {type(id)}")
        if not isinstance(title, str):
            raise TypeError(f"title must be a string, not {type(title)}")
        if not isinstance(description, str):
            raise TypeError(f"description must be a string, not {type(description)}")
        if not isinstance(status, Status):
            raise TypeError(f"status must be a Status, not {type(status)}")
        if not isinstance(priority, Priority):
            raise TypeError(f"priority must be a Priority, not {type(priority)}")
        if not isinstance(duration, Duration):
            raise TypeError(f"duration must be a Duration, not {type(duration)}")
        if not isinstance(charts, list):
            raise TypeError(f"charts must be a list, not {type(charts)}")
        if not all(isinstance(i, str) for i in charts):
            raise TypeError(f"all elements of charts must be a string")
        if not isinstance(tags, list):
            raise TypeError(f"tags must be a list, not {type(tags)}")
        if not all(isinstance(i, str) for i in tags):
            raise TypeError(f"all elements of tags must be a string")
        if not isinstance(relations, dict):
            raise TypeError(f"relations must be a dict, not {type(relations)}")
        if not all(isinstance(k, str) for k in relations.keys()):
            raise TypeError(f"all keys of relations must be a string")
        if not all(isinstance(v, list) for v in relations.values()):
            raise TypeError(f"all values of relations must be a list")
        if not all(all(isinstance(i, str) for i in v) for v in relations.values()):
            raise TypeError(f"all elements of all values of relations must be a string")
        if not isinstance(time_constraint, (TimeConstraint, type(None))):
            raise TypeError(f"time_constraint must be a TimeConstraint or None, not {type(time_constraint)}")
        if not isinstance(user_comment, (str, type(None))):
            raise TypeError(f"user_comment must be a string or None, not {type(user_comment)}")
        if not isinstance(auto_comment, (str, type(None))):
            raise TypeError(f"auto_comment must be a string or None, not {type(auto_comment)}")
        if not isinstance(occlude, bool):
            raise TypeError(f"occlude must be a bool, not {type(occlude)}")

        self.id = id
        self.title = title
        self.description = description
        self.status = status
        self.priority = priority
        self.duration = duration
        self.charts = charts
        self.tags = tags
        self.relations = relations
        self.time_constraint = time_constraint
        self.user_comment = user_comment
        self.auto_comment = auto_comment
        self.occlude = occlude

    @classmethod
    def from_string(cls, line: str, occlude: bool = False) -> 'GianttItem':
        """Parse a line into a GianttItem."""
        line = line.strip()

        # Parse the pre-title section
        pre_title = line[:line.find('"')].strip()
        status_str, id_priority_str, duration_str = parse_pre_title_section(pre_title)
        status = Status(status_str)

        # Parse the title
        title_start = line.find('"')
        title_end = line.find('"', title_start + 1)
        while title_end != -1 and line[title_end - 1] == '\\':
            title_end = line.find('"', title_end + 1)

        if title_end == -1:
            raise ValueError("No ending quote found for title")

        title = json.loads(line[title_start:title_end + 1])
        post_title = line[title_end + 1:].strip()

        # Extract ID and priority
        priority_symbols = ['!!!', '!!', '!', '?', '...', ',,,']
        id_str = id_priority_str
        priority = ''
        for symbol in priority_symbols:
            if id_priority_str.endswith(symbol):
                id_str = id_priority_str[:-len(symbol)]
                priority = symbol
                break
        # must be type Priority
        priority = Priority(priority)

        # Parse duration
        duration = Duration.parse(duration_str)

        # Parse post-title section
        charts_pattern = re.compile(r'^\s*(\{[^}]+\})\s*(.*)$')
        charts_match = charts_pattern.match(post_title)
        if not charts_match:
            raise ValueError("Invalid charts format")

        charts_str = charts_match.group(1)
        remainder = charts_match.group(2)

        # Split remainder into tags, relations, and constraints
        parts = remainder.split('>>>')
        tags_str = parts[0].strip()
        relations_str = parts[1].strip() if len(parts) > 1 else ""

        # Split relations section into relations and time constraints
        constraint_parts = relations_str.split('@@@')
        relations_str = constraint_parts[0].strip()
        time_constraint_str = constraint_parts[1].strip() if len(constraint_parts) > 1 else None

        # Parse charts
        charts = [c.strip().strip('"') for c in charts_str[1:-1].split(",") if c.strip()]

        # Parse tags
        tags = [t.strip() for t in tags_str.split(",") if t.strip()]

        # Parse relations
        relations = {}
        rel_symbols = {r.value: r.name for r in RelationType}

        for symbol, rel_type in rel_symbols.items():
            pattern = f"{symbol}\\[([^]]+)\\]"
            matches = re.findall(pattern, relations_str)
            if matches:
                relations[rel_type] = [t.strip() for t in matches[0].split(",")]

        return cls(
            id=id_str,
            title=title,
            description="",  # Not currently supported
            status=status,
            priority=priority,
            duration=duration,
            charts=charts,
            tags=tags,
            relations=relations,
            time_constraint=time_constraint_str,
            user_comment=None,
            auto_comment=None,
            occlude=occlude
        )


    def to_string(self) -> str:
        charts_str = '{"' + '","'.join(self.charts) + '"}'
        tags_str = ' ' + ','.join(self.tags) if self.tags else ""

        rel_parts = []
        for rel_type, targets in self.relations.items():
            if targets:
                symbol = RelationType[rel_type].value
                rel_parts.append(f"{symbol}[{','.join(targets)}]")
        relations_str = ' >>> ' + ' '.join(rel_parts) if rel_parts else ""

        # JSON encode the title to handle special characters properly
        title_str = json.dumps(self.title)

        user_comment_str = f" # {self.user_comment}" if self.user_comment else ""
        auto_comment_str = f" ### {self.auto_comment}" if self.auto_comment else ""

        # Note: occlusion status is not included in the string representation because it only dictates where the string is saved

        return f"{self.status.value} {self.id}{self.priority.value} {self.duration} {title_str} {charts_str}{tags_str}{relations_str}{user_comment_str}{auto_comment_str}"

    def set_occlude(self, occlude: bool):
        self.occlude = occlude

    def copy(self):
        return GianttItem(
            self.id,
            self.title,
            self.description,
            self.status,
            self.priority,
            self.duration,
            self.charts.copy(),
            self.tags.copy(),
            self.relations.copy(),
            self.time_constraint,
            self.user_comment,
            self.auto_comment,
            self.occlude
        )

class CycleDetectedException(Exception):
    def __init__(self, cycle_items):
        self.cycle_items = cycle_items
        cycle_str = " -> ".join(cycle_items)
        super().__init__(f"Cycle detected in dependencies: {cycle_str}")


class GianttGraph:
    def __init__(self):
        self.items: dict[str, GianttItem] = {}

    def add_item(self, item: GianttItem):
        self.items[item.id] = item

    def find_by_substring(self, substring: str) -> GianttItem:
        matches = [item for item in self.items.values() if substring.lower() in item.title.lower() or substring == item.id]
        if not matches:
            raise ValueError(f"No items with ID '{substring}' or title containing '{substring}' found")
        if len(matches) > 1:
            raise ValueError(f"Multiple matches found: {', '.join(item.id for item in matches)}")
        return matches[0]

    def _safe_topological_sort(self, in_memory_copy=None):
        """
        Performs a safe topological sort that detects cycles and provides detailed error information.

        Args:
            items: Dictionary mapping item IDs to their GianttItem objects
            in_memory_copy: Optional dictionary to use for sorting attempt (to avoid modifying original)

        Returns:
            List of sorted GianttItem objects

        Raises:
            CycleDetectedException: If a dependency cycle is detected, with details about the cycle
        """
        # Build adjacency list for strict relations
        adj_list = {item.id: set() for item in self.items.values()}
        for item in self.items.values():
            for rel_type in ['REQUIRES']:
                if rel_type in item.relations:
                    for target in item.relations[rel_type]:
                        if target not in adj_list:
                            continue # Skip non-existent items
                        adj_list[item.id].add(target)

        # Calculate in-degrees
        in_degree = {node: 0 for node in adj_list}
        for node in adj_list:
            for neighbor in adj_list[node]:
                in_degree[neighbor] = in_degree.get(neighbor, 0) + 1

        # Find nodes with no dependencies
        queue = [node for node, degree in in_degree.items() if degree == 0]
        sorted_items = []
        visited = set()

        while queue:
            node = queue.pop(0)
            sorted_items.append(self.items[node])
            visited.add(node)

            for neighbor in adj_list[node]:
                in_degree[neighbor] -= 1
                if in_degree[neighbor] == 0:
                    queue.append(neighbor)

        # If we haven't visited all nodes, there must be a cycle
        if len(sorted_items) != len(self.items):
            # Find the cycle for better error reporting
            def find_cycle():
                unvisited = set(self.items.keys()) - visited
                stack = []
                path = []

                def dfs(current):
                    if current in stack:
                        cycle_start = stack.index(current)
                        return stack[cycle_start:]
                    if current in visited:
                        return None

                    stack.append(current)
                    for neighbor in adj_list[current]:
                        cycle = dfs(neighbor)
                        if cycle:
                            return cycle
                    stack.pop()
                    return None

                # Start DFS from any unvisited node
                start_node = next(iter(unvisited))
                cycle = dfs(start_node)
                if cycle:
                    # Add one more occurrence of first node to show complete cycle
                    cycle.append(cycle[0])
                return cycle or []

            cycle = find_cycle()
            raise CycleDetectedException(cycle)

        sorted_items.reverse()
        return sorted_items

    def topological_sort(self) -> List[GianttItem]:
        """
        Performs a deterministic topological sort of the graph.
        Returns sorted items in a completely deterministic order.
        """
        # First get basic topological sort
        sorted_items = self._safe_topological_sort()

        # Now within each "level" (items with same dependencies depth),
        # sort by deterministic criteria
        def get_item_sort_key(item):
            return (
                # Primary sort by topological depth
                self._get_dependency_depth(item),
                # Secondary sort by ID (deterministic tie-breaker)
                item.id,
                # Could add more deterministic criteria here
            )

        return sorted(sorted_items, key=get_item_sort_key)

    def _get_dependency_depth(self, item):
        """Get the maximum dependency depth of an item."""
        if 'REQUIRES' not in item.relations:
            return 0

        max_depth = 0
        for dep_id in item.relations['REQUIRES']:
            if dep_id in self.items:
                dep_depth = self._get_dependency_depth(self.items[dep_id])
                max_depth = max(max_depth, dep_depth + 1)
        return max_depth

    def insert_between(self, new_item: GianttItem, before_id: str, after_id: str):
        if before_id not in self.items or after_id not in self.items:
            raise ValueError("Both before and after items must exist")

        before_item = self.items[before_id]
        after_item = self.items[after_id]

        # Update relations
        new_item.relations['REQUIRES'] = [before_id]
        new_item.relations['BLOCKS'] = [after_id]

        # Update existing items
        if 'BLOCKS' in before_item.relations:
            before_item.relations['BLOCKS'].remove(after_id)
            before_item.relations['BLOCKS'].append(new_item.id)

        if 'REQUIRES' in after_item.relations:
            after_item.relations['REQUIRES'].remove(before_id)
            after_item.relations['REQUIRES'].append(new_item.id)

        self.add_item(new_item)

    def included_items(self):
        """Get all items that are not occluded."""
        return {item_id: item for item_id, item in self.items.items() if not item.occlude}

    def occluded_items(self):
        """Get all items that are occluded."""
        return {item_id: item for item_id, item in self.items.items() if item.occlude}

    def copy(self):
        new_graph = GianttGraph()
        for item in self.items.values():
            new_graph.add_item(item.copy())
        return new_graph

    def plus(self, other: 'GianttGraph') -> 'GianttGraph':
        new_graph = self.copy()
        for item in other.items.values():
            new_graph.add_item(item.copy())
        return new_graph

    def __add__(self, other: 'GianttGraph') -> 'GianttGraph':
        return self.plus(other)


@dataclass
class LogEntry:
    """A single log entry recording an event or thought."""
    session: str
    timestamp: datetime
    message: str
    tags: Set[str]
    metadata: Dict[str, str] = field(default_factory=dict)
    occlude: bool = False

    @classmethod
    def create(cls, session_tag: str, message: str, additional_tags: Optional[List[str]] = None, occlude: bool = False) -> 'LogEntry':
        """Create a new log entry with current timestamp."""
        tags = {session_tag}
        if additional_tags:
            tags.update(additional_tags)

        return cls(
            session=session_tag,
            timestamp=datetime.now(timezone.utc),
            message=message,
            tags=tags,
            metadata={},
            occlude=occlude
        )

    def has_tag(self, tag: str) -> bool:
        """Check if entry has a specific tag."""
        return tag in self.tags

    def has_any_tags(self, tags: List[str]) -> bool:
        """Check if entry has any of the specified tags."""
        return bool(self.tags.intersection(tags))

    def has_all_tags(self, tags: List[str]) -> bool:
        """Check if entry has all of the specified tags."""
        return self.tags.issuperset(tags)

    def add_tag(self, tag: str) -> None:
        """Add a tag to the entry."""
        self.tags.add(tag)

    def remove_tag(self, tag: str) -> None:
        """Remove a tag from the entry."""
        self.tags.discard(tag)

    def set_occlude(self, occlude: bool) -> None:
        """Set the occlusion status of the entry."""
        self.occlude = occlude

    def __str__(self):
        return f"{self.timestamp.isoformat()} - {self.message} ({', '.join(self.tags)})"

    def from_dict(data: dict, occlude: bool = False) -> 'LogEntry':
        """Create a LogEntry object from a dictionary."""
        return LogEntry(
            session=data['s'],
            timestamp=datetime.fromisoformat(data['t']),
            message=data['m'],
            tags=set(data['tags']),
            metadata=data.get('meta', {}),
            occlude=occlude
        )

    def from_line(line: str, occlude: bool = False) -> 'LogEntry':
        """Create a LogEntry object from a jsonl line."""
        data = json.loads(line)
        return LogEntry.from_dict(data, occlude)

    def to_dict(self) -> dict:
        """Convert the LogEntry to a dictionary."""
        # occlusion status is not added to the dictionary because it only dictates where the string is saved
        return {
            's': self.session,
            't': self.timestamp.isoformat(),
            'm': self.message,
            'tags': sorted(list(self.tags)),
            'meta': self.metadata
        }

    def to_line(self) -> str:
        """Convert the LogEntry to a jsonl line."""
        # occlusion status is not added to the dictionary because it only dictates where the string is saved
        return json.dumps(self.to_dict(), sort_keys=True)


class LogCollection:
    """A collection of log entries with query capabilities."""

    def __init__(self, entries: Optional[List[LogEntry]] = None):
        self.entries = entries or []

    def add_entry(self, entry: LogEntry) -> None:
        """Add a new entry to the collection."""
        index = self.get_first_index_after_timestamp(entry.timestamp)
        self.entries.insert(index + 1, entry)

    def add_occlude_entry(self, entry: LogEntry) -> None:
        """Add a new entry to the collection ensuring occluded status."""
        entry.occlude = True
        self.add_entry(entry)

    def add_entries(self, entries: List[LogEntry]) -> None:
        self.entries.extend(entries)
        self.sort()

    def create_entry(self, session_tag: str, message: str, additional_tags: Optional[List[str]] = None, occlude: bool = False) -> LogEntry:
        """Create and add a new entry."""
        entry = LogEntry.create(session_tag, message, additional_tags, occlude)
        self.add_entry(entry)
        return entry

    def sort(self) -> None:
        """Sort entries by timestamp."""
        self.entries.sort(key=lambda e: e.timestamp)

    def get_by_session(self, session_tag: str) -> List[LogEntry]:
        """Get all entries with a specific session tag."""
        return [entry for entry in self.entries if entry.session == session_tag]

    def get_by_tags(self, tags: List[str], require_all: bool = False) -> List[LogEntry]:
        """Get entries with specified tags.

        Args:
            tags: List of tags to match
            require_all: If True, entries must have all tags; if False, any tag matches
        """
        if require_all:
            return [entry for entry in self.entries if entry.has_all_tags(tags)]
        return [entry for entry in self.entries if entry.has_any_tags(tags)]

    def get_by_date_range(self, start: datetime, end: Optional[datetime] = None) -> List[LogEntry]:
        """Get entries within a date range."""
        end = end or datetime.now(timezone.utc)
        return [
            entry for entry in self.entries 
            if start <= entry.timestamp <= end
        ]

    def get_by_substring(self, substring: str) -> List[LogEntry]:
        """Get entries with a specific substring in the message."""
        return [entry for entry in self.entries if substring.lower() in entry.message.lower()]

    def get_first_index_after_timestamp(self, timestamp: datetime) -> int:
        """Get the index of the first entry after a timestamp."""
        if not self.entries:
            return 0
        if timestamp >= self.entries[-1].timestamp:
            return len(self.entries) - 1
        if timestamp < self.entries[0].timestamp:
            return 0
        low = 0
        high = len(self.entries) - 1
        while low < high:
            mid = (low + high) // 2
            if self.entries[mid].timestamp < timestamp:
                low = mid + 1
            else:
                high = mid
        return low

    def include_entries(self) -> List[LogEntry]:
        """Get all entries that are not occluded."""
        return [entry for entry in self.entries if not entry.occlude]

    def occluded_entries(self) -> List[LogEntry]:
        """Get all entries that are occluded."""
        return [entry for entry in self.entries if entry.occlude]

    def __iter__(self):
        return iter(self.entries)


class IssueType(Enum):
    DANGLING_REFERENCE = "dangling_reference"
    ORPHANED_ITEM = "orphaned_item"
    INCOMPLETE_CHAIN = "incomplete_chain"
    CHART_INCONSISTENCY = "chart_inconsistency"
    TAG_INCONSISTENCY = "tag_inconsistency"
    
    @classmethod
    def from_string(cls, value: str) -> 'IssueType':
        """Convert a string to an IssueType."""
        for issue_type in cls:
            if issue_type.value == value:
                return issue_type
        raise ValueError(f"Invalid issue type: {value}")

@dataclass
class Issue:
    type: IssueType
    item_id: str
    message: str
    related_ids: List[str]
    suggested_fix: Optional[str] = None

class GianttDoctor:
    def __init__(self, graph: 'GianttGraph'):
        self.graph = graph
        self.issues: List[Issue] = []
        self.fixed_issues: List[Issue] = []

    def quick_check(self) -> int:
        """Run a quick check and return number of issues found."""
        self.issues = []
        self._check_references()
        return len(self.issues)

    def full_diagnosis(self) -> List[Issue]:
        """Run all checks and return detailed issues."""
        self.issues = []
        self._check_references()
        # Not clear that any commented below are actually issues
        # self._check_orphans()
        self._check_chains()
        # self._check_charts()
        # self._check_tags()
        return self.issues
        
    def get_issues_by_type(self, issue_type: IssueType) -> List[Issue]:
        """Get all issues of a specific type."""
        return [issue for issue in self.issues if issue.type == issue_type]
    
    def fix_issues(self, issue_type: Optional[IssueType] = None, item_id: Optional[str] = None) -> List[Issue]:
        """Fix issues of a specific type or for a specific item."""
        # Filter issues to fix
        issues_to_fix = self.issues
        if issue_type:
            issues_to_fix = [issue for issue in issues_to_fix if issue.type == issue_type]
        if item_id:
            issues_to_fix = [issue for issue in issues_to_fix if issue.item_id == item_id]
            
        fixed = []
        for issue in issues_to_fix:
            if self._fix_issue(issue):
                fixed.append(issue)
                
        # Remove fixed issues from the issues list
        for issue in fixed:
            if issue in self.issues:
                self.issues.remove(issue)
                
        self.fixed_issues.extend(fixed)
        return fixed
    
    def _fix_issue(self, issue: Issue) -> bool:
        """Fix a specific issue. Returns True if fixed, False otherwise."""
        if issue.type == IssueType.DANGLING_REFERENCE:
            return self._fix_dangling_reference(issue)
        elif issue.type == IssueType.INCOMPLETE_CHAIN:
            return self._fix_incomplete_chain(issue)
        # Add more issue type handlers as needed
        return False
        
    def _fix_dangling_reference(self, issue: Issue) -> bool:
        """Fix a dangling reference issue."""
        item = self.graph.items.get(issue.item_id)
        if not item:
            return False
            
        # Find the relation type and target from the message
        rel_type = None
        target = None
        for rel_name in RelationType._member_names_:
            if rel_name.lower() in issue.message.lower():
                rel_type = rel_name
                break
                
        if not rel_type:
            return False
            
        # Extract the target ID from the message
        import re
        match = re.search(r"non-existent item '([^']+)'", issue.message)
        if not match:
            return False
            
        target = match.group(1)
        
        # Remove the dangling reference
        if rel_type in item.relations and target in item.relations[rel_type]:
            item.relations[rel_type].remove(target)
            if not item.relations[rel_type]:
                del item.relations[rel_type]
            return True
            
        return False
        
    def _fix_incomplete_chain(self, issue: Issue) -> bool:
        """Fix an incomplete chain issue."""
        if not issue.related_ids or not issue.suggested_fix:
            return False
            
        item = self.graph.items.get(issue.item_id)
        related_item = self.graph.items.get(issue.related_ids[0])
        if not item or not related_item:
            return False
            
        # Parse the suggested fix to determine what to do
        parts = issue.suggested_fix.split()
        if len(parts) < 4:
            return False
            
        target_id = parts[2]
        action = parts[3]
        rel_type = parts[4].upper() if len(parts) > 4 else None
        
        if target_id != issue.item_id and target_id != issue.related_ids[0]:
            return False
            
        if "add" in action.lower() and rel_type:
            target_item = self.graph.items.get(target_id)
            if not target_item:
                return False
                
            # Add the relation
            target_item.relations.setdefault(rel_type, [])
            if parts[5] not in target_item.relations[rel_type]:
                target_item.relations[rel_type].append(parts[5])
            return True
            
        return False

    def _check_references(self):
        """Check for dangling references in relations."""
        for item_id, item in self.graph.items.items():
            for rel_type, targets in item.relations.items():
                for target in targets:
                    if target not in self.graph.items:
                        self.issues.append(Issue(
                            type=IssueType.DANGLING_REFERENCE,
                            item_id=item_id,
                            message=f"References non-existent item '{target}' in {rel_type.lower()} relation",
                            related_ids=[target],
                            suggested_fix=f"giantt modify {item_id} --remove {rel_type.lower()} {target}"
                        ))

    def _check_orphans(self):
        """Find items with no incoming or outgoing relations."""
        for item_id, item in self.graph.items.items():
            has_incoming = any(
                target == item_id
                for other in self.graph.items.values()
                for targets in other.relations.values()
                for target in targets
            )
            has_outgoing = bool(item.relations)

            if not has_incoming and not has_outgoing:
                self.issues.append(Issue(
                    type=IssueType.ORPHANED_ITEM,
                    item_id=item_id,
                    message="Item has no relations to other items",
                    related_ids=[],
                    suggested_fix="Consider connecting this item to related tasks"
                ))

    def _check_chains(self):
        """Check for incomplete dependency chains."""
        blocks_map = {
            item_id: set(targets)
            for item_id, item in self.graph.items.items()
            for targets in [item.relations.get('BLOCKS', [])]
        }
        requires_map = {
            item_id: set(targets)
            for item_id, item in self.graph.items.items()
            for targets in [item.relations.get('REQUIRES', [])]
        }
        sufficient_map = {
            item_id: set(targets)
            for item_id, item in self.graph.items.items()
            for targets in [item.relations.get('SUFFICIENT', [])]
        }
        anyof_map = {
            item_id: set(targets)
            for item_id, item in self.graph.items.items()
            for targets in [item.relations.get('ANY', [])]
        }

        # Check for items that block something but aren't required by it or vice versa
        for item_id, blocks_items in blocks_map.items():
            for blocked in blocks_items:
                if blocked in self.graph.items:
                    if item_id not in requires_map.get(blocked, set()):
                        self.issues.append(Issue(
                            type=IssueType.INCOMPLETE_CHAIN,
                            item_id=item_id,
                            message=f"Item blocks '{blocked}' but isn't required by it",
                            related_ids=[blocked],
                            suggested_fix=f"giantt modify {blocked} --add requires {item_id}"
                        ))
        for item_id, requires_items in requires_map.items():
            for required in requires_items:
                if required in self.graph.items:
                    if item_id not in blocks_map.get(required, set()):
                        self.issues.append(Issue(
                            type=IssueType.INCOMPLETE_CHAIN,
                            item_id=item_id,
                            message=f"Item requires '{required}' but isn't blocked by it",
                            related_ids=[required],
                            suggested_fix=f"giantt modify {required} --add blocks {item_id}"
                        ))
        # Check for items that are sufficient for something but aren't in an any relation with it, or vice versa
        for item_id, sufficient_items in sufficient_map.items():
            for sufficient in sufficient_items:
                if sufficient in self.graph.items:
                    if item_id not in anyof_map.get(sufficient, set()):
                        self.issues.append(Issue(
                            type=IssueType.INCOMPLETE_CHAIN,
                            item_id=item_id,
                            message=f"Item is sufficient for '{sufficient}' but doesn't have any-of relation with it",
                            related_ids=[sufficient],
                            suggested_fix=f"giantt modify {sufficient} --add any {item_id}"
                        ))
        for item_id, anyof_items in anyof_map.items():
            for anyof_item in anyof_items:
                if anyof_item in self.graph.items:
                    if item_id not in sufficient_map.get(anyof_item, set()):
                        self.issues.append(Issue(
                            type=IssueType.INCOMPLETE_CHAIN,
                            item_id=item_id,
                            message=f"Item has any-of relation with '{anyof_item}' but isn't sufficient for it",
                            related_ids=[anyof_item],
                            suggested_fix=f"giantt modify {anyof_item} --add sufficient {item_id}"
                        ))

    def _check_charts(self):
        """Check for chart consistency issues."""
        # Find all unique charts
        all_charts = set()
        chart_items: Dict[str, Set[str]] = {}

        for item_id, item in self.graph.items.items():
            for chart in item.charts:
                all_charts.add(chart)
                if chart not in chart_items:
                    chart_items[chart] = set()
                chart_items[chart].add(item_id)

        # Check for items that should probably be in certain charts
        for chart in all_charts:
            chart_set = chart_items[chart]
            for item_id in chart_set:
                item = self.graph.items[item_id]
                # Check if any required items or blocked items in this chart
                # aren't also in this chart
                related_items = set(item.relations.get('REQUIRES', []) + 
                                 item.relations.get('BLOCKS', []))
                for related_id in related_items:
                    if (related_id in self.graph.items and 
                        related_id not in chart_set and
                        any(c == chart for c in self.graph.items[related_id].charts)):
                        self.issues.append(Issue(
                            type=IssueType.CHART_INCONSISTENCY,
                            item_id=related_id,
                            message=f"Item is related to items in chart '{chart}' but isn't in it",
                            related_ids=[item_id],
                            suggested_fix=""
                        ))

    def _check_tags(self):
        """Check for tag consistency issues."""
        # Find all unique tags
        all_tags = set()
        tag_items: Dict[str, Set[str]] = {}

        for item_id, item in self.graph.items.items():
            for tag in item.tags:
                all_tags.add(tag)
                if tag not in tag_items:
                    tag_items[tag] = set()
                tag_items[tag].add(item_id)

        # Check for items that should probably have certain tags
        for tag in all_tags:
            tag_set = tag_items[tag]
            for item_id in tag_set:
                item = self.graph.items[item_id]
                # Check if any required items with this tag aren't also tagged
                related_items = set(item.relations.get('REQUIRES', []) + 
                                 item.relations.get('BLOCKS', []))
                for related_id in related_items:
                    if (related_id in self.graph.items and 
                        related_id not in tag_set and
                        any(t == tag for t in self.graph.items[related_id].tags)):
                        self.issues.append(Issue(
                            type=IssueType.TAG_INCONSISTENCY,
                            item_id=related_id,
                            message=f"Item is related to items with tag '{tag}' but doesn't have it",
                            related_ids=[item_id],
                            suggested_fix=""
                        ))
