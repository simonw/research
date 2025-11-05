"""
SQLite Query Linter

A wrapper around sqlite3 that provides configurable linting rules for SQL queries.
Helps catch common mistakes and bad practices before executing queries.
"""

import sqlite3
import re
from typing import List, Optional, Tuple, Any
from dataclasses import dataclass
from enum import Enum


class LintLevel(Enum):
    """Severity level for linting issues"""
    ERROR = "error"      # Query should not be executed
    WARNING = "warning"  # Query has issues but can execute
    INFO = "info"        # Informational message


@dataclass
class LintIssue:
    """Represents a linting issue found in a query"""
    level: LintLevel
    rule_name: str
    message: str
    query: str
    position: Optional[int] = None  # Character position in query if applicable


class LintRule:
    """Base class for linting rules"""

    def __init__(self, name: str, level: LintLevel = LintLevel.ERROR):
        self.name = name
        self.level = level

    def check(self, query: str) -> List[LintIssue]:
        """
        Check a query for issues.

        Args:
            query: SQL query string to check

        Returns:
            List of LintIssue objects found (empty if no issues)
        """
        raise NotImplementedError("Subclasses must implement check()")


class InvalidCastTypeRule(LintRule):
    """Check for invalid type names in CAST expressions"""

    # Valid SQLite affinity types
    VALID_TYPES = {
        'INTEGER', 'INT', 'TINYINT', 'SMALLINT', 'MEDIUMINT', 'BIGINT',
        'UNSIGNED BIG INT', 'INT2', 'INT8',
        'TEXT', 'CLOB', 'CHARACTER', 'VARCHAR', 'VARYING CHARACTER',
        'NCHAR', 'NATIVE CHARACTER', 'NVARCHAR',
        'REAL', 'DOUBLE', 'DOUBLE PRECISION', 'FLOAT',
        'NUMERIC', 'DECIMAL', 'BOOLEAN', 'DATE', 'DATETIME',
        'BLOB'
    }

    # Common mistakes
    INVALID_MAPPINGS = {
        'STRING': 'TEXT',
        'STR': 'TEXT',
        'LONG': 'INTEGER',
        'BOOL': 'INTEGER',
    }

    def __init__(self):
        super().__init__("invalid_cast_type", LintLevel.ERROR)

    def check(self, query: str) -> List[LintIssue]:
        issues = []
        # Pattern to match CAST expressions
        cast_pattern = r'CAST\s*\(\s*[^)]+\s+AS\s+([A-Za-z][A-Za-z0-9\s]*)\s*\)'

        for match in re.finditer(cast_pattern, query, re.IGNORECASE):
            type_name = match.group(1).strip().upper()

            # Check if it's a valid type
            if type_name not in self.VALID_TYPES:
                suggestion = self.INVALID_MAPPINGS.get(type_name)
                if suggestion:
                    message = f"Invalid type '{type_name}' in CAST. Did you mean '{suggestion}'?"
                else:
                    message = f"Invalid type '{type_name}' in CAST. Valid types include: TEXT, INTEGER, REAL, BLOB, NUMERIC"

                issues.append(LintIssue(
                    level=self.level,
                    rule_name=self.name,
                    message=message,
                    query=query,
                    position=match.start()
                ))

        return issues


class SelectStarRule(LintRule):
    """Warn about SELECT * queries"""

    def __init__(self, level: LintLevel = LintLevel.WARNING):
        super().__init__("select_star", level)

    def check(self, query: str) -> List[LintIssue]:
        issues = []
        # Look for SELECT * patterns
        if re.search(r'\bSELECT\s+\*\s+FROM\b', query, re.IGNORECASE):
            issues.append(LintIssue(
                level=self.level,
                rule_name=self.name,
                message="SELECT * can be inefficient and fragile. Consider specifying columns explicitly.",
                query=query
            ))
        return issues


class MissingWhereClauseRule(LintRule):
    """Warn about UPDATE/DELETE without WHERE clause"""

    def __init__(self, level: LintLevel = LintLevel.WARNING):
        super().__init__("missing_where", level)

    def check(self, query: str) -> List[LintIssue]:
        issues = []

        # Check DELETE without WHERE
        if re.search(r'\bDELETE\s+FROM\s+\w+\s*(?:;|$)', query, re.IGNORECASE):
            issues.append(LintIssue(
                level=self.level,
                rule_name=self.name,
                message="DELETE without WHERE clause will remove all rows. Add WHERE clause or use DELETE FROM table WHERE 1=1 to confirm.",
                query=query
            ))

        # Check UPDATE without WHERE
        if re.search(r'\bUPDATE\s+\w+\s+SET\s+.+?(?:;|$)(?!.*WHERE)', query, re.IGNORECASE | re.DOTALL):
            if 'WHERE' not in query.upper():
                issues.append(LintIssue(
                    level=self.level,
                    rule_name=self.name,
                    message="UPDATE without WHERE clause will modify all rows. Add WHERE clause or use WHERE 1=1 to confirm.",
                    query=query
                ))

        return issues


class InvalidFunctionRule(LintRule):
    """Check for common function name mistakes"""

    INVALID_FUNCTIONS = {
        'CONCAT': 'Use || operator instead',
        'LEN': 'Use LENGTH() instead',
        'ISNULL': 'Use IFNULL() or COALESCE() instead',
        'GETDATE': 'Use CURRENT_TIMESTAMP or datetime() instead',
        'NOW': 'Use CURRENT_TIMESTAMP or datetime() instead',
    }

    def __init__(self):
        super().__init__("invalid_function", LintLevel.ERROR)

    def check(self, query: str) -> List[LintIssue]:
        issues = []

        for func, suggestion in self.INVALID_FUNCTIONS.items():
            pattern = r'\b' + func + r'\s*\('
            if re.search(pattern, query, re.IGNORECASE):
                issues.append(LintIssue(
                    level=self.level,
                    rule_name=self.name,
                    message=f"Invalid function {func}(). {suggestion}",
                    query=query
                ))

        return issues


class DoubleQuotesForStringsRule(LintRule):
    """Warn about using double quotes for string literals"""

    def __init__(self, level: LintLevel = LintLevel.WARNING):
        super().__init__("double_quotes_strings", level)

    def check(self, query: str) -> List[LintIssue]:
        issues = []

        # Look for double-quoted strings (not identifiers)
        # This is a simplification - a full parser would be more accurate
        # Double quotes are for identifiers in SQLite, single quotes for strings
        pattern = r'"[^"]*"'
        matches = list(re.finditer(pattern, query))

        if matches and not re.search(r'\bCREATE\b', query, re.IGNORECASE):
            issues.append(LintIssue(
                level=self.level,
                rule_name=self.name,
                message="Use single quotes for string literals. Double quotes are for identifiers in SQLite.",
                query=query
            ))

        return issues


class LintingCursor:
    """Cursor wrapper that lints queries before execution"""

    def __init__(self, cursor: sqlite3.Cursor, rules: List[LintRule],
                 raise_on_error: bool = True, raise_on_warning: bool = False):
        self._cursor = cursor
        self._rules = rules
        self._raise_on_error = raise_on_error
        self._raise_on_warning = raise_on_warning
        self.last_issues: List[LintIssue] = []

    def _lint_query(self, query: str) -> List[LintIssue]:
        """Run all linting rules on a query"""
        all_issues = []
        for rule in self._rules:
            issues = rule.check(query)
            all_issues.extend(issues)
        return all_issues

    def _check_and_raise(self, query: str):
        """Lint query and raise exception if configured to do so"""
        self.last_issues = self._lint_query(query)

        errors = [i for i in self.last_issues if i.level == LintLevel.ERROR]
        warnings = [i for i in self.last_issues if i.level == LintLevel.WARNING]

        if errors and self._raise_on_error:
            error_msgs = '\n'.join(f"  - {issue.message}" for issue in errors)
            raise LintError(f"Query failed linting:\n{error_msgs}", errors)

        if warnings and self._raise_on_warning:
            warning_msgs = '\n'.join(f"  - {issue.message}" for issue in warnings)
            raise LintWarning(f"Query has warnings:\n{warning_msgs}", warnings)

    def execute(self, sql: str, parameters: tuple = ()):
        """Execute query after linting"""
        self._check_and_raise(sql)
        return self._cursor.execute(sql, parameters)

    def executemany(self, sql: str, seq_of_parameters):
        """Execute query multiple times after linting"""
        self._check_and_raise(sql)
        return self._cursor.executemany(sql, seq_of_parameters)

    def executescript(self, sql_script: str):
        """Execute script after linting"""
        self._check_and_raise(sql_script)
        return self._cursor.executescript(sql_script)

    # Delegate other cursor methods
    def __getattr__(self, name):
        return getattr(self._cursor, name)

    def __iter__(self):
        return iter(self._cursor)

    def __next__(self):
        return next(self._cursor)


class LintingConnection:
    """Connection wrapper that creates linting cursors"""

    def __init__(self, database: str, rules: Optional[List[LintRule]] = None,
                 raise_on_error: bool = True, raise_on_warning: bool = False,
                 **kwargs):
        """
        Create a linting SQLite connection.

        Args:
            database: Database file path or ":memory:"
            rules: List of linting rules to apply (uses defaults if None)
            raise_on_error: Raise exception on ERROR level issues
            raise_on_warning: Raise exception on WARNING level issues
            **kwargs: Additional arguments passed to sqlite3.connect()
        """
        self._connection = sqlite3.connect(database, **kwargs)
        self._rules = rules if rules is not None else self._default_rules()
        self._raise_on_error = raise_on_error
        self._raise_on_warning = raise_on_warning

    @staticmethod
    def _default_rules() -> List[LintRule]:
        """Get default linting rules"""
        return [
            InvalidCastTypeRule(),
            InvalidFunctionRule(),
            SelectStarRule(LintLevel.WARNING),
            MissingWhereClauseRule(LintLevel.WARNING),
            DoubleQuotesForStringsRule(LintLevel.WARNING),
        ]

    def cursor(self) -> LintingCursor:
        """Create a linting cursor"""
        return LintingCursor(
            self._connection.cursor(),
            self._rules,
            self._raise_on_error,
            self._raise_on_warning
        )

    def execute(self, sql: str, parameters: tuple = ()):
        """Execute query directly on connection"""
        cursor = self.cursor()
        return cursor.execute(sql, parameters)

    def executemany(self, sql: str, seq_of_parameters):
        """Execute query multiple times"""
        cursor = self.cursor()
        return cursor.executemany(sql, seq_of_parameters)

    def executescript(self, sql_script: str):
        """Execute SQL script"""
        cursor = self.cursor()
        return cursor.executescript(sql_script)

    # Delegate other connection methods
    def __getattr__(self, name):
        return getattr(self._connection, name)

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        return self._connection.__exit__(exc_type, exc_val, exc_tb)


class LintError(Exception):
    """Exception raised when query fails linting with ERROR level"""
    def __init__(self, message: str, issues: List[LintIssue]):
        super().__init__(message)
        self.issues = issues


class LintWarning(Exception):
    """Exception raised when query fails linting with WARNING level"""
    def __init__(self, message: str, issues: List[LintIssue]):
        super().__init__(message)
        self.issues = issues


def connect(database: str, rules: Optional[List[LintRule]] = None,
            raise_on_error: bool = True, raise_on_warning: bool = False,
            **kwargs) -> LintingConnection:
    """
    Create a linting SQLite connection (convenience function).

    Args:
        database: Database file path or ":memory:"
        rules: List of linting rules to apply (uses defaults if None)
        raise_on_error: Raise exception on ERROR level issues
        raise_on_warning: Raise exception on WARNING level issues
        **kwargs: Additional arguments passed to sqlite3.connect()

    Returns:
        LintingConnection instance

    Example:
        >>> import sqlite_linter
        >>> conn = sqlite_linter.connect(":memory:")
        >>> conn.execute("SELECT CAST(value AS STRING) FROM table")
        LintError: Invalid type 'STRING' in CAST. Did you mean 'TEXT'?
    """
    return LintingConnection(database, rules, raise_on_error, raise_on_warning, **kwargs)
