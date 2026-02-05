from typing import List, Dict, Tuple, Optional, Set
import click
import shutil
import tempfile
from pathlib import Path
import re
import json
import os

from giantt_core import (
    GianttGraph, GianttItem, RelationType,
    Status, Priority, Duration,
    LogEntry, LogCollection,
    Issue, IssueType, GianttDoctor,
    CycleDetectedException
)

def get_default_giantt_path(filename: str = 'items.txt', occlude: bool = False) -> str:
    """Get the default path for Giantt files."""
    # whether it's the occlude or include directory
    filepath = Path('occlude' if occlude else 'include') / filename
    # First check for local .giantt directory
    local_path = Path.cwd() / '.giantt' / filepath
    if local_path.exists():
        return str(local_path)

    # Fall back to home directory
    home_path = Path.home() / '.giantt' / filepath
    if home_path.exists():
        return str(home_path)

    # If neither exists, raise an error
    raise click.ClickException(f"No Giantt {filepath} found. Please run 'giantt init' or 'giantt init --dev' first.")

def increment_backup_name(filepath: str) -> str:
    """Increment the backup name for a file."""
    backup_num = 1
    while True:
        backup_path = f"{filepath}.{backup_num}.backup"
        if not os.path.exists(backup_path):
            return backup_path
        backup_num += 1

def most_recent_backup_name(filepath: str) -> str:
    """Get the most recent backup name for a file."""
    # get list of backups by listing the directory
    backups = os.listdir(os.path.dirname(filepath))
    # sort key that gets the number in the backup name as an integer
    key = lambda x: int(x.split('.')[-2]) if x.endswith('.backup') else 0
    for backup in reversed(sorted(backups, key=key)):
        if backup.startswith(f"{os.path.basename(filepath)}.") and backup.endswith(".backup"):
            return os.path.join(os.path.dirname(filepath), backup)

def parse_include_directives(filepath: str) -> List[str]:
    """Parse include directives from a file.
    
    Include directives should be at the top of the file in the format:
    #include path/to/file.txt
    
    Returns:
        List of file paths to include
    """
    includes = []
    try:
        with open(filepath, 'r') as f:
            for line in f:
                line = line.strip()
                if not line or not line.startswith('#include '):
                    break  # Only process directives at the top
                include_path = line[9:].strip()  # Remove '#include ' prefix
                includes.append(include_path)
    except FileNotFoundError:
        click.echo(f"Warning: Include file not found: {filepath}", err=True)
    return includes

def load_graph_from_file(filepath: str, loaded_files: Optional[Set[str]] = None) -> GianttGraph:
    """Load a graph from a file, processing include directives.
    
    Args:
        filepath: Path to the file to load
        loaded_files: Set of files already loaded (to prevent circular includes)
        
    Returns:
        GianttGraph object
    """
    if loaded_files is None:
        loaded_files = set()
    
    # Prevent circular includes
    if filepath in loaded_files:
        click.echo(f"Warning: Circular include detected for {filepath}, skipping", err=True)
        return GianttGraph()
    
    loaded_files.add(filepath)
    
    # Create a backup of the file first if it exists
    if os.path.exists(filepath):
        shutil.copyfile(filepath, increment_backup_name(filepath))
    else:
        click.echo(f"Warning: File not found: {filepath}, skipping", err=True)
        return GianttGraph()
    
    # Process include directives
    includes = parse_include_directives(filepath)
    
    # Create the graph
    graph = GianttGraph()
    
    # Load included files first
    for include_path in includes:
        # Handle relative paths
        if not os.path.isabs(include_path):
            base_dir = os.path.dirname(filepath)
            include_path = os.path.join(base_dir, include_path)
        
        try:
            include_graph = load_graph_from_file(include_path, loaded_files)
            graph = graph + include_graph
        except Exception as e:
            click.echo(f"Warning: Error loading include {include_path}: {e}", err=True)
    
    # Now load the main file
    with open(filepath, 'r') as f:
        for line in f:
            line = line.strip()
            if line and not line.startswith('#'):
                try:
                    item = GianttItem.from_string(line, occlude='occlude' in filepath)
                    graph.add_item(item)
                except ValueError as e:
                    click.echo(f"Warning: Skipping invalid line: {e}", err=True)
    
    return graph

def load_logs_from_file(filepath: str) -> LogCollection:
    # create a backup of the file first
    shutil.copyfile(filepath, increment_backup_name(filepath))
    logs = LogCollection()
    with open(filepath, 'r') as f:
        for line in f:
            line = line.strip()
            if line and not line.startswith('#'):
                try:
                    log = LogEntry.from_line(line, occlude='occlude' in filepath)
                    logs.add_entry(log)
                except json.JSONDecodeError as e:
                    click.echo(f"Warning: Skipping invalid log line: {e}", err=True)
    return logs

def load_graph(filepath: str, occlude_filepath: str) -> GianttGraph:
    """Load a graph from main and occluded files, processing includes."""
    loaded_files = set()
    return load_graph_from_file(filepath, loaded_files) + load_graph_from_file(occlude_filepath, loaded_files)

def load_logs(filepath: str, occlude_filepath: str) -> LogCollection:
    logs = load_logs_from_file(filepath)
    logs.add_entries(load_logs_from_file(occlude_filepath)) # occlude status is picked up from the file path
    return logs

def load_graph_and_logs(items_file: str, occlude_items_file: str, logs_file: str, occlude_logs_file: str):
    """Load GianttGraph and LogCollection objects from files."""
    return load_graph(items_file, occlude_items_file), load_logs(logs_file, occlude_logs_file)

def create_banner(text: str, padding_h: int = 5, padding_v: int = 1):
    """Create a banner of hash characters in a box with text centered inside."""
    lines = text.split('\n')
    max_length = max(len(line) for line in lines)
    banner_len = max_length + 2 * padding_h  # Account for horizontal padding
    top_bottom_border = "#" * (banner_len + 2)  # Add border around text

    banner = top_bottom_border + "\n"

    # Add vertical padding lines
    empty_line = "#" + " " * banner_len + "#"
    for _ in range(padding_v):
        banner += empty_line + "\n"

    # Add text lines, centered
    for line in lines:
        padding = max_length - len(line)
        left_padding = padding // 2
        right_padding = padding - left_padding
        banner += f"#{' ' * padding_h}{' ' * left_padding}{line}{' ' * right_padding}{' ' * padding_h}#\n"

    # Add vertical padding lines
    for _ in range(padding_v):
        banner += empty_line + "\n"

    banner += top_bottom_border + "\n"

    return banner

ITEMS_FILE_BANNER = (
    create_banner(
        'Giantt Items\n'
        'This file contains all include Giantt items in topological\n'
        f'order according to the REQUIRES ({RelationType.REQUIRES.value}) relation.\n'
        'You can use #include directives at the top of this file\n'
        'to include other Giantt item files.\n'
        'Edit this file manually at your own risk.'
    )
)

ITEMS_ARCHIVE_FILE_BANNER = (
    create_banner(
        'Giantt Occluded Items\n'
        'This file contains all occluded Giantt items in topological\n'
        f'order according to the REQUIRES ({RelationType.REQUIRES.value}) relation.\n'
        'Edit this file manually at your own risk.'
    )
)

def save_graph_files(filepath: str, occlude_filepath: str, graph: GianttGraph):
    """
    Safely saves all graph items, split appropriately between
    include and occlude files, by first performing a sort in memory
    and using a transaction-like write to temporary files to
    ensure consistency.

    Args:
        filepath: Path to the items file to save
        occlude_filepath: Path to the occluded items file to save
        graph: GianttGraph object containing items to save

    Raises:
        CycleDetectedException: If dependencies contain a cycle
        ValueError: If dependencies reference non-existent items
    """
    try:
        # First perform the sort in memory to check for issues
        sorted_items = graph.topological_sort()

        # Create temporary files
        temp_include = filepath + '.temp'
        temp_occlude = occlude_filepath + '.temp'

        # Write to temporary files
        with open(temp_include, "w") as f:
            f.write(ITEMS_FILE_BANNER + "\n")
            for item in sorted_items:
                if not item.occlude:
                    f.write(item.to_string() + "\n")

        with open(temp_occlude, "w") as f:
            f.write(ITEMS_ARCHIVE_FILE_BANNER + "\n")
            for item in sorted_items:
                if item.occlude:
                    f.write(item.to_string() + "\n")

        # If we get here, both writes succeeded, so rename temp files
        os.replace(temp_include, filepath)
        os.replace(temp_occlude, occlude_filepath)

        # If most recent backup is identical to the new file, remove it
        for backed_up_file in [filepath, occlude_filepath]:
            most_recent_backup = most_recent_backup_name(backed_up_file)
            if most_recent_backup:
                with open(backed_up_file, 'r') as f:
                    new_contents = f.read()
                with open(most_recent_backup, 'r') as f:
                    old_contents = f.read()
                if new_contents == old_contents:
                    os.remove(most_recent_backup)

        # Run a quick health check
        run_quick_check(graph)

    except (CycleDetectedException, ValueError) as e:
        # Clean up temp files if they exist
        for temp_file in [temp_include, temp_occlude]:
            try:
                os.remove(temp_file)
            except OSError:
                pass
        raise click.ClickException(str(e))

def save_log_files(filepath: str, occlude_filepath: str, logs: LogCollection):
    """
    Safely saves all log entries, split appropriately between
    include and occlude files, with transaction-like behavior.

    Args:
        filepath: Path to the logs file to save
        occlude_filepath: Path to the occlude logs file to save
        logs: LogCollection object containing entries to save
    """
    try:
        # Create temporary files
        temp_include = filepath + '.temp'
        temp_occlude = occlude_filepath + '.temp'

        # Write include logs to temporary file
        with open(temp_include, 'w') as f:
            for log in logs:
                if not log.occlude:
                    f.write(log.to_line() + '\n')

        # Write occluded logs to temporary file
        with open(temp_occlude, 'w') as f:
            for log in logs:
                if log.occlude:
                    f.write(log.to_line() + '\n')

        # If we get here, both writes succeeded, so rename temp files
        os.replace(temp_include, filepath)
        os.replace(temp_occlude, occlude_filepath)

        # If most recent backup is identical to the new file, remove it
        for backed_up_file in [filepath, occlude_filepath]:
            most_recent_backup = most_recent_backup_name(backed_up_file)
            if most_recent_backup:
                with open(backed_up_file, 'r') as f:
                    new_contents = f.read()
                with open(most_recent_backup, 'r') as f:
                    old_contents = f.read()
                if new_contents == old_contents:
                    os.remove(most_recent_backup)

    except Exception as e:
        # Clean up temp files if they exist
        for temp_file in [temp_include, temp_occlude]:
            try:
                os.remove(temp_file)
            except OSError:
                pass
        raise click.ClickException(f"Error saving log files: {str(e)}")

def run_quick_check(graph: GianttGraph) -> None:
    """Run a quick health check after operations."""
    doctor = GianttDoctor(graph)
    issues = doctor.quick_check()
    if issues > 0:
        click.echo(
            click.style(
                f"\n{issues} or more warnings. Run 'giantt doctor' for details.",
                fg='yellow'
            )
        )

@click.group()
def cli():
    """Giantt command line utility for managing task dependencies."""
    pass

@cli.command()
@click.option('--dev', is_flag=True, help='Initialize for development')
@click.option('--data-dir', type=click.Path(), help='Custom data directory location')
def init(dev: bool, data_dir: str):
    """Initialize Giantt directory structure and files."""

    # Determine base directory
    if dev:
        # Use local directory in dev mode
        base_dir = Path.cwd() / '.giantt'
    else:
        # Use ~/.giantt in normal mode
        base_dir = Path.home() / '.giantt'

    # Override with custom location if specified
    if data_dir:
        base_dir = Path(data_dir)

    # Create directory structure
    dirs = [
        base_dir / 'include',
        base_dir / 'occlude'
    ]

    for dir_path in dirs:
        dir_path.mkdir(parents=True, exist_ok=True)

    # Create initial files if they don't exist
    files = {
        base_dir / 'include' / 'items.txt': ITEMS_FILE_BANNER,
        base_dir / 'include' / 'metadata.json': "{}",
        base_dir / 'include' / 'logs.jsonl': "",
        base_dir / 'occlude' / 'items.txt': ITEMS_ARCHIVE_FILE_BANNER,
        base_dir / 'occlude' / 'metadata.json': "{}",
        base_dir / 'occlude' / 'logs.jsonl': ""
    }

    already_exists = set()

    for file_path, initial_content in files.items():
        if file_path.exists():
            already_exists.add(file_path)
        else:
            with open(file_path, 'w') as f:
                f.write(initial_content)

    if already_exists == set(files.keys()):
        click.echo(f"Giantt is already initialized at {base_dir}. Enjoy!")
    else:
        click.echo(f"Initialized Giantt at {base_dir}")

def show_one_item(graph, substring):
    # If there is an exact match to an ID, select that item. Otherwise, find by title substring
    error = None
    item = None
    if substring in graph.items:
        item = graph.items[substring]
    else:
        try:
            item = graph.find_by_substring(substring)
        except ValueError as e:
            error = str(e)
            click.echo(f"{error}")
    if item:
        click.echo(f"Title: {item.title}")
        click.echo(f"ID: {item.id}")
        click.echo(f"Status: {item.status.name}")
        click.echo(f"Priority: {item.priority.name}")
        click.echo(f"Duration: {item.duration}")
        click.echo(f"Charts: {', '.join(item.charts)}")
        click.echo(f"Tags: {', '.join(item.tags) if item.tags else 'None'}")
        click.echo(f"Time Constraint: {item.time_constraint}")
        click.echo("Relations:")
        for rel_type, targets in item.relations.items():
            click.echo(f"    - {rel_type}: {', '.join(targets)}")
        click.echo(f"Comment: {item.user_comment}")
        click.echo(f"Auto Comment: {item.auto_comment}")

def show_chart(graph, substring):
    chart_items = {}
    for item in graph.items.values():
        for chart in item.charts:
            if substring.lower() in chart.lower():
                if chart not in chart_items:
                    chart_items[chart] = []
                chart_items[chart].append(item)
    for chart, items in chart_items.items():
        click.echo(f"Chart '{chart}':")
        for item in items:
            click.echo(f"  - {item.id} {item.title}")
    if not chart_items:
        click.echo(f"No items found in chart '{substring}'")


def show_logs(logs, substring):
    # search in logs and log sessions
    log_entries = logs.get_by_session(substring)
    if log_entries:
        click.echo(f"Logs for session '{substring}':")
        for entry in log_entries:
            click.echo(f"  - {entry}")
    else:
        click.echo(f"No logs found for session '{substring}'")
    log_entries = logs.get_by_substring(substring)
    if log_entries:
        click.echo(f"Logs matching '{substring}':")
        for entry in log_entries:
            click.echo(f"  - {entry}")

@cli.command()
@click.option('--file', '-f', default=None, help='Giantt items file to use')
@click.option('--occlude-file', '-a', default=None, help='Giantt occluded items file to use')
@click.option('--log-file', '-l', default=None, help='Giantt log file to use')
@click.option('--occlude-log-file', '-al', default=None, help='Giantt occlude log file to use')
@click.option('--chart', is_flag=True, default=False, help='Search in chart names')
@click.option('--log', is_flag=True, default=False, help='Search in logs and log sessions')
@click.argument('substring')
def show(file: str, occlude_file: str, log_file: str, occlude_log_file: str, chart: bool, log: bool, substring: str):
    """Show details of an item matching the substring."""
    file = file or get_default_giantt_path()
    occlude_file = occlude_file or get_default_giantt_path(occlude=True)
    log_file = log_file or get_default_giantt_path('logs.jsonl')
    occlude_log_file = occlude_log_file or get_default_giantt_path('logs.jsonl', occlude=True)

    graph, log_collection = load_graph_and_logs(file, occlude_file, log_file, occlude_log_file)
    if not chart and not log:
        show_one_item(graph, substring)
    if chart:
        show_chart(graph, substring)
    if log:
        show_logs(log_collection, substring)

@cli.command()
@click.option('--file', '-f', default=None, help='Logs file to use')
@click.option('--occlude-file', '-a', default=None, help='Occluded logs file to use')
@click.argument('session')
@click.option('--tags', help='Additional comma-separated tags')
@click.argument('message')
def log(file: str, occlude_file: str, session: str, tags: str, message: str):
    """Create a log entry with session tag and message.

    session: The session tag for the log entry.
    tags: Optional additional tags for the log entry.
    message: The message to log.

    The log entry will be appended to logs.jsonl in the include directory.
    Each entry includes:
    - Timestamp
    - Session tag
    - Additional tags (optional)
    - Message

    Example usage:
    $ giantt log rim0 --tags planning,ideas "Initial brainstorming session"
    """
    file = file or get_default_giantt_path('logs.jsonl')
    occlude_file = occlude_file or get_default_giantt_path('logs.jsonl', occlude=True)
    logs = load_logs(file, occlude_file)
    logs.create_entry(session, message, tags.split(',') if tags else None)
    save_log_files(file, occlude_file, logs)
    click.echo(f"Log entry created with session tag '{session}'")

@cli.command()
@click.option('--file', '-f', default=None, help='Giantt items file to use')
@click.option('--occlude-file', '-a', default=None, help='Giantt occluded items file to use')
@click.argument('substring')
@click.argument('new_status', type=click.Choice([s.name for s in Status]))
def set_status(file: str, occlude_file: str, substring: str, new_status: str):
    """Set the status of an item."""
    file = file or get_default_giantt_path()
    occlude_file = occlude_file or get_default_giantt_path(occlude=True)
    graph = load_graph(file, occlude_file)
    try:
        item = graph.find_by_substring(substring)
    except ValueError as e:
        raise click.ClickException(str(e))
    item.status = Status[new_status]
    save_graph_files(file, occlude_file, graph)
    click.echo(f"Set status of item '{item.id}' to {new_status}")

@cli.command()
@click.option('--file', '-f', default=None, help='Giantt items file to use')
@click.option('--occlude-file', '-a', default=None, help='Giantt occluded items file to use')
@click.argument('id')
@click.argument('title')
@click.option('--duration', default='1d', help='Duration (e.g., 1d, 2w, 3mo)')
@click.option('--priority', type=click.Choice([p.name for p in Priority]), default='NEUTRAL')
@click.option('--charts', help='Comma-separated list of chart names')
@click.option('--tags', help='Comma-separated list of tags')
@click.option('--status', type=click.Choice([s.name for s in Status]), default='NOT_STARTED')
@click.option('--requires', help='Comma-separated list of item IDs that this item requires')
@click.option('--any-of', help='Comma-separated list of item IDs that are individually sufficient for this item')
def add(file: str, occlude_file: str, id: str, title: str, duration: str, priority: str, 
        charts: str, tags: str, status: str, requires: str, any_of: str):
    """Add a new item to the Giantt chart."""
    file = file or get_default_giantt_path()
    occlude_file = occlude_file or get_default_giantt_path(occlude=True)
    graph = load_graph(file, occlude_file)

    # Validate ID is unique and string search for this ID or title won't conflict with other titles
    for item in graph.items.values():
        if item.id == id:
            raise click.ClickException(f"Item ID '{id}' already exists\n"
                                       f"Existing item: {item.id} - {item.title}")
        if id.lower() in item.title.lower():
            raise click.ClickException(f"Item ID '{id}' conflicts with title of another item\n"
                                       f"Conflicting item: {item.id} - {item.title}")
        if title.lower() in item.title.lower():
            raise click.ClickException(f"Title '{title}' conflicts with title of another item\n"
                                       f"Conflicting item: {item.id} - {item.title}")

    # Create relations dict
    relations = {}
    if requires:
        relations['REQUIRES'] = requires.split(',')

    if any_of:
        relations['ANYOF'] = any_of.split(',')

    # Create new item
    try:
        item = GianttItem(
            id=id,
            title=title,
            description="",  # Not currently supported
            status=Status[status],
            priority=Priority[priority],
            duration=Duration.parse(duration),
            charts=charts.split(',') if charts else [],
            tags=tags.split(',') if tags else [],
            relations=relations,
            time_constraint=None,
            user_comment=None,
            auto_comment=None
        )
    except ValueError as e:
        raise click.ClickException(f"Error: {str(e)}")

    # Add item
    graph.add_item(item)

    # Try to save, catching potential cycle issues
    try:
        save_graph_files(file, occlude_file, graph)
        click.echo(f"Added item '{id}'")
    except CycleDetectedException as e:
        click.echo(f"Error: {str(e)}", err=True)
        click.echo("\nThe new item would create a dependency cycle. Please revise the relations.", err=True)
    except ValueError as e:
        click.echo(f"Error: {str(e)}", err=True)
        click.echo("\nPlease fix invalid dependencies before adding the item.", err=True)

@cli.command()
@click.option('--file', '-f', default=None, help='Giantt items file to use')
@click.option('--occlude-file', '-a', default=None, help='Giantt occluded items file to use')
@click.option('--force', '-f', is_flag=True, help='Force removal without confirmation')
@click.argument('item_id')
@click.option('--keep-relations', is_flag=True, default=False, help='Keep relations to other items')
def remove(file: str, occlude_file: str, force: bool, item_id: str, keep_relations: bool):
    """Remove an item from the Giantt chart and clean up relations."""

    file = file or get_default_giantt_path()
    occlude_file = occlude_file or get_default_giantt_path(occlude=True)

    graph = load_graph(file, occlude_file)

    # Find the item
    if item_id not in graph.items:
        click.echo(click.style(f"Error: Item '{item_id}' not found.", fg='red'), err=True)
        return

    item = graph.items[item_id]

    if not force:
        # Display item details
        click.echo(click.style("\nItem to be removed:", fg='yellow', bold=True))
        click.echo(f"  ID: {item.id}")
        click.echo(f"  Title: {item.title}")
        click.echo(f"  Status: {item.status.name}")
        click.echo(f"  Priority: {item.priority.name}")
        click.echo(f"  Duration: {item.duration}")
        click.echo(f"  Charts: {', '.join(item.charts) if item.charts else 'None'}")
        click.echo(f"  Tags: {', '.join(item.tags) if item.tags else 'None'}")

        # Count affected relations
        relation_counts = {rel: 0 for rel in RelationType._member_names_}
        for other_item in graph.items.values():
            for rel_type, targets in other_item.relations.items():
                if item_id in targets:
                    relation_counts[rel_type] += 1

        # Display relation impact
        if any(relation_counts.values()):
            click.echo(click.style("\nRelations that will be affected" + (" (but not removed):" if keep_relations else ":"), fg='yellow', bold=True))
            for rel_type, count in relation_counts.items():
                if count > 0:
                    click.echo(f"  {rel_type}: {count} references removed")
        else:
            click.echo(click.style("\nNo relations will be affected.", fg='cyan'))

        # Confirm deletion
        confirm = click.prompt("\nConfirm removal? (y/N)", default="N").strip().lower()
        if confirm != 'y':
            click.echo(click.style("Aborted. No changes made.", fg='cyan'))
            return

    # Remove the item from the graph
    del graph.items[item_id]

    if not keep_relations:
        # Remove references in other items
        for other_item in graph.items.values():
            for rel_type in other_item.relations:
                other_item.relations[rel_type] = [t for t in other_item.relations[rel_type] if t != item_id]

    # Save changes
    save_graph_files(file, occlude_file, graph)

    click.echo(click.style(f"\nSuccessfully removed '{item_id}' and cleaned up relations.", fg='green'))

@cli.command()
@click.option('--file', '-f', default=None, help='Giantt items file to use')
@click.option('--occlude-file', '-a', default=None, help='Giantt occluded items file to use')
@click.option('--add', is_flag=True, help='Add a relation')
@click.option('--remove', is_flag=True, help='Remove a relation')
@click.argument('substring')
@click.argument('property')
@click.argument('value')
def modify(file: str, occlude_file: str, add: bool, remove: bool, substring: str, property: str, value: str):
    """Modify any property of a Giantt item."""
    file = file or get_default_giantt_path()
    occlude_file = occlude_file or get_default_giantt_path(occlude=True)
    graph = load_graph(file, occlude_file)
    try:
        item = graph.find_by_substring(substring)
    except ValueError as e:
        raise click.ClickException(str(e))

    if add and remove:
        raise click.ClickException("Cannot add and remove a relation at the same time")

    # Handle relations
    relation_types = {r.name.lower() for r in RelationType}
    if (add or remove) and property.lower() not in relation_types:
        raise click.ClickException(f"Invalid relation type. Must be one of: {', '.join(relation_types)}")

    if property.lower() in relation_types:
        relation_type = property.upper()
        targets = [t.strip() for t in value.split(',') if t.strip()]

        if add:
            item.relations.setdefault(relation_type, []).extend(targets)
        elif remove:
            if relation_type not in item.relations:
                raise click.ClickException(f"No {relation_type} relations to remove")
            item.relations[relation_type] = [t for t in item.relations.get(relation_type, []) if t not in targets]

        # Prevent cycles when modifying REQUIRES relations
        if relation_type == 'REQUIRES':
            temp_graph = graph.copy()
            temp_graph.items[item.id] = item.copy()
            try:
                temp_graph.topological_sort()
            except CycleDetectedException as e:
                raise click.ClickException(f"Adding these requirements would create a cycle: {' -> '.join(e.cycle_items)}")

    # Handle standard properties
    else:
        if property == 'title':
            item.title = value
        elif property == 'duration':
            item.duration = Duration.parse(value)
        elif property == 'priority':
            try:
                item.priority = Priority[value.upper()]
            except KeyError:
                raise click.ClickException(f"Invalid priority. Must be one of: {', '.join(p.name for p in Priority)}")
        elif property == 'status':
            try:
                item.status = Status[value.upper()]
            except KeyError:
                raise click.ClickException(f"Invalid status. Must be one of: {', '.join(s.name for s in Status)}")
        elif property == 'charts':
            item.charts = [c.strip() for c in value.split(',') if c.strip()]
        elif property == 'tags':
            item.tags = [t.strip() for t in value.split(',') if t.strip()]
        else:
            raise click.ClickException(
                f"Unknown property. Must be one of: title, duration, priority, status, charts, {', '.join(relation_types)}, or tags")

    save_graph_files(file, occlude_file, graph)
    click.echo(f"Modified {property} of item '{item.id}'")


@cli.command()
@click.option('--file', '-f', default=None, help='Giantt items file to use')
@click.option('--occlude-file', '-a', default=None, help='Giantt occluded items file to use')
def sort(file: str, occlude_file: str):
    """Sort items in topological order and save."""
    file = file or get_default_giantt_path()
    occlude_file = occlude_file or get_default_giantt_path(occlude=True)
    graph = load_graph(file, occlude_file)
    try:
        save_graph_files(file, occlude_file, graph)
        click.echo("Successfully sorted and saved items.")
    except CycleDetectedException as e:
        click.echo(f"Error: {str(e)}", err=True)
        click.echo("\nPlease resolve the cycle before sorting.", err=True)
    except ValueError as e:
        click.echo(f"Error: {str(e)}", err=True)
        click.echo("\nPlease fix invalid dependencies before sorting.", err=True)

#giantt touch command: same as sort (just load and save) but for both graph and logs
@cli.command()
@click.option('--file', '-f', default=None, help='Giantt items file to use')
@click.option('--recursive', '-r', is_flag=True, help='Show recursive includes')
def includes(file: str, recursive: bool):
    """Show the include structure of a Giantt items file."""
    file = file or get_default_giantt_path()
    
    def process_file(filepath: str, depth: int = 0, visited: Optional[Set[str]] = None) -> None:
        if visited is None:
            visited = set()
            
        if filepath in visited:
            click.echo(f"{'  ' * depth}└─ {filepath} (circular include, skipping)")
            return
            
        visited.add(filepath)
        
        if not os.path.exists(filepath):
            click.echo(f"{'  ' * depth}└─ {filepath} (file not found)")
            return
            
        click.echo(f"{'  ' * depth}└─ {filepath}")
        
        if recursive:
            includes = parse_include_directives(filepath)
            for include_path in includes:
                # Handle relative paths
                if not os.path.isabs(include_path):
                    base_dir = os.path.dirname(filepath)
                    include_path = os.path.join(base_dir, include_path)
                
                process_file(include_path, depth + 1, visited)
    
    process_file(file)

@cli.command()
@click.option('--file', '-f', default=None, help='Giantt items file to use')
@click.option('--occlude-file', '-a', default=None, help='Giantt occluded items file to use')
@click.option('--log-file', '-l', default=None, help='Giantt log file to use')
@click.option('--occlude-log-file', '-al', default=None, help='Giantt occlude log file to use')
def touch(file: str, occlude_file: str, log_file: str, occlude_log_file: str):
    """Touch items and logs files to trigger a reload and save."""
    file = file or get_default_giantt_path()
    occlude_file = occlude_file or get_default_giantt_path(occlude=True)
    log_file = log_file or get_default_giantt_path('logs.jsonl')
    occlude_log_file = occlude_log_file or get_default_giantt_path('logs.jsonl', occlude=True)
    graph, logs = load_graph_and_logs(file, occlude_file, log_file, occlude_log_file)
    save_graph_files(file, occlude_file, graph)
    save_log_files(log_file, occlude_log_file, logs)
    click.echo("Touched items and logs files")

@cli.command()
@click.option('--file', '-f', default=None, help='Giantt items file to use')
@click.option('--occlude-file', '-a', default=None, help='Giantt occluded items file to use')
@click.argument('new_id')
@click.argument('before_id')
@click.argument('after_id')
@click.option('--charts', help='Comma-separated list of charts')
@click.option('--tags', help='Comma-separated list of tags')
@click.option('--duration', default='1d', help='Duration (e.g., 1d, 2w, 3mo2w5d3s)')
@click.option('--priority', type=click.Choice([p.name for p in Priority]), 
              default='NEUTRAL', help='Priority level')
def insert(file: str, occlude_file: str, new_id: str, before_id: str, after_id: str,
          charts: str, tags: str, duration: str, priority: str):
    """Insert a new item between two existing items."""
    file = file or get_default_giantt_path()
    occlude_file = occlude_file or get_default_giantt_path(occlude=True)
    graph = load_graph(file, occlude_file)

    try:
        new_item = GianttItem(
            id=new_id,
            priority=Priority[priority],
            duration=duration,
            charts=charts.split(',') if charts else [],
            tags=tags.split(',') if tags else []
        )
    except ValueError as e:
        raise click.ClickException(f"Error: {str(e)}")

    graph.insert_between(new_item, before_id, after_id)
    save_graph_files(file, occlude_file, graph)

@cli.group()
def occlude():
    """Occlude items or logs."""
    # This is not a placeholder function, it's a click command group
    pass

@occlude.command() # This is part of the occlude command group
@click.option('--file', '-f', default=None, help='Giantt items file to use')
@click.option('--occlude-file', '-a', default=None, help='Giantt occluded items file to use')
@click.option('--tag', '-t', multiple=True, help='Occlude items with specific tags')
@click.option('--dry-run/--no-dry-run', default=False, help='Show what would be occluded without making changes')
@click.argument('identifiers', nargs=-1)
def items(file: str, occlude_file: str, tag: Tuple[str, ...], dry_run: bool, identifiers: Tuple[str, ...]):
    """Occlude Giantt items.

    Can occlude by specifying IDs directly and/or by providing tags.

    Examples:
        # Occlude specific items
        giantt occluded items item1 item2

        # Occlude items by tag
        giantt occluded items -t project1 -t phase1

        # Do a dry run first
        giantt occluded items --dry-run -t project1
    """
    # Get source and destination files
    items_file = file or get_default_giantt_path('items.txt')
    items_occlude = occlude_file or get_default_giantt_path(occlude=True)

    # Load current items
    graph = load_graph(items_file, items_occlude)

    # Load metadata (we will come back to metadata)
    # try:
    #     with open(metadata_file) as f:
    #         metadata = json.load(f)
    # except (FileNotFoundError, json.JSONDecodeError):
    #     metadata = {}


    # try: (we will come back to metadata)
    #     with open(metadata_occlude) as f:
    #         occluded_metadata = json.load(f)
    # except (FileNotFoundError, json.JSONDecodeError):
    #     occluded_metadata = {}

    # Find items to occlude
    to_occlude = set()

    # Add items by ID
    for id in identifiers:
        if id in graph.included_items():
            to_occlude.add(id)
        else:
            click.echo(f"Warning: Item '{id}' not found in included items", err=True)

    # Add items by tag
    for t in tag:
        for item in graph.included_items():
            if t in graph.items[item].tags:
                to_occlude.add(item)

    if not to_occlude:
        click.echo("No included items found to occlude")
        return

    # In dry-run mode, just show what would be occluded
    if dry_run:
        click.echo("The following items would be occluded:")
        for item_id in sorted(to_occlude):
            item = graph.items[item_id]
            click.echo(f"  • {item_id}: {item.title}")
        return

    # Set occlude status for items
    for id in to_occlude:
        graph.items[id].set_occlude(True)

    # Save updated items
    save_graph_files(items_file, items_occlude, graph)
    click.echo(f"Occluded {len(to_occlude)} item" + ("s" if len(to_occlude) != 1 else ""))

@occlude.command() # This is part of the occlude command group
@click.option('--file', '-f', default=None, help='Giantt logs file to use')
@click.option('--occlude-file', '-a', default=None, help='Giantt occlude logs file to use')
@click.option('--tag', '-t', multiple=True, help='Occlude logs with specific tags')
@click.option('--dry-run/--no-dry-run', default=False, help='Show what would be occluded without making changes')
@click.argument('identifiers', nargs=-1)
def logs(file: str, occlude_file: str, tag: Tuple[str, ...], dry_run: bool, identifiers: Tuple[str, ...]):
    """Occlude log entries.

    Can occlude by specifying IDs directly and/or by providing tags.

    Examples:
        # Occlude specific logs
        giantt occlude logs log1 log2

        # Occlude logs by tag
        giantt occlude logs -t debug -t test

        # Do a dry run first
        giantt occlude logs --dry-run -t debug
    """
    # Get source and destination files
    logs_file = file or get_default_giantt_path('logs.jsonl')
    logs_occlude = occlude_file or get_default_giantt_path('logs.jsonl', occlude=True)

    # Load current logs
    logs = load_logs(logs_file, logs_occlude)

    # Find logs to occlude
    to_occlude = []

    for log in logs.include_entries():
        should_occlude = False

        # Check if log has matching ID
        if log.session in identifiers:
            should_occlude = True

        # Check if log has matching tag
        if any(t in log.tags for t in tag):
            should_occlude = True

        if should_occlude:
            to_occlude.append(log)

    if not to_occlude:
        click.echo("No include logs found to occlude")
        return

    # In dry-run mode, just show what would be occluded
    if dry_run:
        click.echo("The following logs would be occluded:")
        for log in to_occlude:
            # time needs to be formatted to be human-readable
            click.echo(f"  • {log.message} ({', '.join(log.tags)}) {log.timestamp.strftime('%Y-%m-%d %H:%M:%S')}")
        return

    # Set occlude status for logs
    for log in to_occlude:
        log.set_occlude(True)

    # Save updated logs
    save_log_files(logs_file, logs_occlude, logs)

    click.echo(f"Occluded {len(to_occlude)} log" + ("s" if len(to_occlude) != 1 else ""))

@cli.command()
@click.option('--file', '-f', default=None, help='Giantt items file to use')
@click.argument('include_path')
def add_include(file: str, include_path: str):
    """Add an include directive to a Giantt items file."""
    file = file or get_default_giantt_path()
    
    if not os.path.exists(file):
        raise click.ClickException(f"File not found: {file}")
    
    # Read the current file content
    with open(file, 'r') as f:
        content = f.readlines()
    
    # Find where to insert the include directive
    insert_pos = 0
    for i, line in enumerate(content):
        if line.strip().startswith('#include '):
            insert_pos = i + 1
        elif line.strip() and not line.strip().startswith('#'):
            break
    
    # Create a backup
    shutil.copyfile(file, increment_backup_name(file))
    
    # Insert the include directive
    content.insert(insert_pos, f"#include {include_path}\n")
    
    # Write the updated content
    with open(file, 'w') as f:
        f.writelines(content)
    
    click.echo(f"Added include directive for {include_path} to {file}")

@cli.command()
@click.option('--yes', '-y', is_flag=True, help='Skip confirmation prompt')
@click.option('--keep', '-k', default=3, help='Number of recent backups to keep')
def clean(yes: bool, keep: int):
    """Clean up backup files, keeping only the most recent few backups.

    By default, keeps the 3 most recent backups and renames them to .1.backup (oldest),
    .2.backup, and .3.backup (newest).
    """

    # Get paths using the existing function
    try:
        include_items_path = get_default_giantt_path()
        occlude_items_path = get_default_giantt_path(occlude=True)
    except click.ClickException:
        click.echo("Giantt is not initialized. Run 'giantt init' or giantt init --dev' first,\nor navigate to your local dev directory.")
        return

    # Extract directories
    include_dir = Path(include_items_path).parent
    occlude_dir = Path(occlude_items_path).parent

    items_pattern = re.compile(r'items\.txt\.(\d+)\.backup$')
    logs_pattern = re.compile(r'logs\.jsonl\.(\d+)\.backup$')

    # Find all backup files
    backup_files = []

    for directory in [include_dir, occlude_dir]:
        for filename in directory.glob('*.backup'):
            if items_pattern.search(filename.name) or logs_pattern.search(filename.name):
                backup_files.append(filename)

    if not backup_files:
        click.echo("No backup files found.")
        return

    # Group by base filename
    grouped_backups = {}
    for filepath in backup_files:
        base_name = filepath.name.split('.', 1)[0] + '.' + filepath.name.split('.', 2)[1]  # e.g., "items.txt" or "logs.jsonl"
        directory = filepath.parent
        key = (directory, base_name)

        if key not in grouped_backups:
            grouped_backups[key] = []

        backup_num = int(filepath.name.split('.')[-2])  # Extract backup number
        grouped_backups[key].append((backup_num, filepath))

    # Sort each group by backup number (descending) and determine files to delete
    to_delete = []
    to_rename = {}

    for (directory, base_name), backups in grouped_backups.items():
        # Sort by backup number (highest first)
        backups.sort(key=lambda x: x[0], reverse=True)

        # Keep only the most recent 'keep' backups
        if len(backups) > keep:
            to_delete.extend([filepath for _, filepath in backups[keep:]])

        # Rename the kept backups to .1.backup, .2.backup, etc.
        # Oldest backup gets .1.backup, newest gets .<keep>.backup
        kept_backups = backups[:keep]
        kept_backups.reverse()  # Reverse so oldest is first

        for i, (_, filepath) in enumerate(kept_backups):
            new_filename = directory / f"{base_name}.{i+1}.backup"
            to_rename[filepath] = new_filename

    # Show summary and confirm
    click.echo(f"Found {len(backup_files)} backup files across all directories.")
    click.echo(f"Will keep {min(keep, len(backup_files))} most recent backups of each file.")
    if not to_delete:
        click.echo("No files to delete.")
        return
    click.echo(f"Will delete {len(to_delete)} old backup files.")

    if to_delete and not yes:
        click.confirm("Do you want to proceed?", abort=True)

    # Perform operations
    temp_dir = Path(tempfile.mkdtemp())
    try:
        # First move files to be deleted to a temp directory
        for filepath in to_delete:
            shutil.move(str(filepath), str(temp_dir / filepath.name))

        # Then rename the files to be kept
        # We need to handle rename conflicts carefully
        rename_plan = []
        for old_path, new_path in to_rename.items():
            if old_path.exists():  # May have been deleted in cleanup
                # If target exists, need temp name
                if new_path.exists() and old_path != new_path:
                    temp_path = temp_dir / f"temp_{old_path.name}"
                    rename_plan.append((old_path, temp_path))
                    rename_plan.append((temp_path, new_path))
                else:
                    rename_plan.append((old_path, new_path))

        # Execute renames in order
        for old_path, new_path in rename_plan:
            shutil.move(str(old_path), str(new_path))

        # Finally delete the temp directory with all files to be deleted
        shutil.rmtree(temp_dir)

        click.echo("Backup cleanup completed successfully!")

    except Exception as e:
        click.echo(f"Error during cleanup: {e}", err=True)
        click.echo("Attempting to recover...", err=True)
        try:
            # Try to restore any moved files
            for filepath in to_delete:
                temp_path = temp_dir / filepath.name
                if temp_path.exists():
                    shutil.move(str(temp_path), str(filepath))
            shutil.rmtree(temp_dir)
            click.echo("Recovery completed.", err=True)
        except Exception as recovery_error:
            click.echo(f"Recovery failed: {recovery_error}", err=True)
            click.echo(f"Some files may have been moved to temporary directory: {temp_dir}", err=True)

@cli.group()
@click.option('--file', '-f', default=None, help='Giantt items file to use')
@click.option('--occlude-file', '-a', default=None, help='Giantt occluded items file to use')
@click.pass_context
def doctor(ctx, file: str, occlude_file: str):
    """Check the health of the Giantt graph and fix issues."""
    ctx.ensure_object(dict)
    ctx.obj['file'] = file or get_default_giantt_path()
    ctx.obj['occlude_file'] = occlude_file or get_default_giantt_path(occlude=True)
    ctx.obj['graph'] = load_graph(ctx.obj['file'], ctx.obj['occlude_file'])
    ctx.obj['doctor'] = GianttDoctor(ctx.obj['graph'])

@doctor.command('check')
@click.pass_context
def doctor_check(ctx):
    """Check the health of the Giantt graph and report issues."""
    doctor = ctx.obj['doctor']
    issues = doctor.full_diagnosis()

    if not issues:
        click.echo(click.style("✓ Graph is healthy!", fg='green'))
        return

    # Group issues by type
    issues_by_type: Dict[IssueType, List[Issue]] = {}
    for issue in issues:
        if issue.type not in issues_by_type:
            issues_by_type[issue.type] = []
        issues_by_type[issue.type].append(issue)

    # Print issues
    click.echo(click.style(f"\nFound {len(issues)} issue" + ("s" if len(issues) != 1 else "") + ":", fg='yellow'))
    for issue_type, type_issues in issues_by_type.items():
        click.echo(f"\n{issue_type.value} ({len(type_issues)} issues):")
        for issue in type_issues:
            click.echo(f"  • {issue.item_id}: {issue.message}")
            if issue.suggested_fix:
                click.echo(f"    Suggested fix: {issue.suggested_fix}")

@doctor.command('fix')
@click.option('--type', '-t', 'issue_type', help='Type of issue to fix (e.g., dangling_reference)')
@click.option('--item', '-i', help='Fix issues for a specific item ID')
@click.option('--all', '-a', is_flag=True, help='Fix all fixable issues')
@click.option('--dry-run', is_flag=True, help='Show what would be fixed without making changes')
@click.pass_context
def doctor_fix(ctx, issue_type: str, item: str, all: bool, dry_run: bool):
    """Fix issues in the Giantt graph.
    
    Examples:
        # Fix all dangling references
        giantt doctor fix --type dangling_reference
        
        # Fix all issues for a specific item
        giantt doctor fix --item item123
        
        # Fix all fixable issues
        giantt doctor fix --all
        
        # Do a dry run first
        giantt doctor fix --all --dry-run
    """
    doctor = ctx.obj['doctor']
    graph = ctx.obj['graph']
    file = ctx.obj['file']
    occlude_file = ctx.obj['occlude_file']
    
    # Run diagnosis first
    issues = doctor.full_diagnosis()
    
    if not issues:
        click.echo(click.style("✓ Graph is healthy! No issues to fix.", fg='green'))
        return
        
    # Filter issues based on options
    issues_to_fix = []
    
    if issue_type:
        try:
            issue_type_enum = IssueType.from_string(issue_type)
            issues_to_fix = doctor.get_issues_by_type(issue_type_enum)
            if not issues_to_fix:
                click.echo(f"No issues of type '{issue_type}' found.")
                return
        except ValueError:
            valid_types = [t.value for t in IssueType]
            click.echo(f"Invalid issue type: {issue_type}. Valid types are: {', '.join(valid_types)}")
            return
    elif item:
        issues_to_fix = [i for i in issues if i.item_id == item]
        if not issues_to_fix:
            click.echo(f"No issues found for item '{item}'.")
            return
    elif all:
        issues_to_fix = issues
    else:
        click.echo("Please specify --type, --item, or --all to indicate which issues to fix.")
        return
        
    # Show what would be fixed
    click.echo(click.style(f"\nFound {len(issues_to_fix)} issue(s) that can be fixed:", fg='yellow'))
    for issue in issues_to_fix:
        click.echo(f"  • {issue.item_id}: {issue.message}")
        if issue.suggested_fix:
            click.echo(f"    Suggested fix: {issue.suggested_fix}")
            
    if dry_run:
        click.echo("\nDry run - no changes made.")
        return
        
    # Confirm before fixing
    if not click.confirm("\nDo you want to fix these issues?"):
        click.echo("Aborted. No changes made.")
        return
        
    # Fix issues
    fixed_issues = doctor.fix_issues(
        issue_type=issue_type_enum if issue_type else None,
        item_id=item
    )
    
    if fixed_issues:
        # Save changes
        save_graph_files(file, occlude_file, graph)
        click.echo(click.style(f"\nSuccessfully fixed {len(fixed_issues)} issue(s):", fg='green'))
        for issue in fixed_issues:
            click.echo(f"  • {issue.item_id}: {issue.message}")
    else:
        click.echo("\nNo issues were fixed. Some issues may require manual intervention.")

@doctor.command('list-types')
def doctor_list_types():
    """List all available issue types that can be fixed."""
    click.echo("Available issue types:")
    for issue_type in IssueType:
        click.echo(f"  • {issue_type.value}")

if __name__ == '__main__':
    cli()
