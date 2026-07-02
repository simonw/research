"""Build a deterministic test SQLite database plus a question/gold-answer
dataset for evaluating datasette-agent's read-only SQL answering.

Gold answers are computed by executing SQL against the generated database,
so they are correct by construction. Each dataset item carries:

- question: natural language question
- gold_sql: a query that produces the answer (shown in GEPA feedback)
- gold_answer: short canonical answer string
- checks: list of values (numbers/strings) that must appear in the
  agent's user-visible output for the answer to count as correct

Usage: python make_dataset.py  (writes books.db and dataset.json)
"""

import json
import random
import sqlite3
from pathlib import Path

HERE = Path(__file__).parent
DB_PATH = HERE / "books.db"
DATASET_PATH = HERE / "dataset.json"

rng = random.Random(42)

FIRST = [
    "Alice", "Bob", "Carmen", "Deepak", "Elena", "Farid", "Grace", "Hiro",
    "Ines", "Jamal", "Katya", "Liam", "Mona", "Nadia", "Omar", "Priya",
    "Quinn", "Rosa", "Sven", "Tara", "Uma", "Viktor", "Wen", "Ximena",
    "Yusuf", "Zoe",
]
LAST = [
    "Anderson", "Brennan", "Castillo", "Dubois", "Eriksen", "Fontaine",
    "Gupta", "Haddad", "Ivanova", "Johansson", "Kimura", "Lombardi",
    "Marino", "Novak", "Okafor", "Petrov", "Quiroga", "Rahman", "Silva",
    "Takahashi",
]
CITIES = [
    "Portland", "Austin", "Chicago", "Denver", "Seattle", "Boston",
    "Madison", "Tucson",
]
COUNTRIES = ["UK", "USA", "Japan", "Nigeria", "Argentina", "France"]
GENRES = [
    "Science Fiction", "Mystery", "Romance", "History", "Poetry",
    "Biography",
]
TITLE_A = [
    "The Silent", "A Distant", "The Last", "Beneath the", "Shadows of",
    "The Glass", "Songs of", "The Iron", "Whispers of", "The Painted",
]
TITLE_B = [
    "River", "Mountain", "Library", "Garden", "Empire", "Harbor",
    "Winter", "Telescope", "Orchard", "Lighthouse",
]


def build_db():
    if DB_PATH.exists():
        DB_PATH.unlink()
    db = sqlite3.connect(DB_PATH)
    db.executescript(
        """
        CREATE TABLE authors (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            country TEXT NOT NULL,
            birth_year INTEGER NOT NULL
        );
        CREATE TABLE books (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            author_id INTEGER NOT NULL REFERENCES authors(id),
            genre TEXT NOT NULL,
            price REAL NOT NULL,
            published_year INTEGER NOT NULL,
            pages INTEGER
        );
        CREATE TABLE customers (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT,
            city TEXT NOT NULL,
            joined_date TEXT NOT NULL
        );
        CREATE TABLE orders (
            id INTEGER PRIMARY KEY,
            customer_id INTEGER NOT NULL REFERENCES customers(id),
            order_date TEXT NOT NULL,
            status TEXT NOT NULL
        );
        CREATE TABLE order_items (
            id INTEGER PRIMARY KEY,
            order_id INTEGER NOT NULL REFERENCES orders(id),
            book_id INTEGER NOT NULL REFERENCES books(id),
            quantity INTEGER NOT NULL,
            unit_price REAL NOT NULL
        );
        """
    )

    # Authors
    used_names = set()
    for i in range(1, 13):
        while True:
            name = f"{rng.choice(FIRST)} {rng.choice(LAST)}"
            if name not in used_names:
                used_names.add(name)
                break
        db.execute(
            "INSERT INTO authors VALUES (?, ?, ?, ?)",
            (i, name, rng.choice(COUNTRIES), rng.randint(1935, 1990)),
        )

    # Books - unique titles
    titles = set()
    for i in range(1, 41):
        while True:
            title = f"{rng.choice(TITLE_A)} {rng.choice(TITLE_B)}"
            if title not in titles:
                titles.add(title)
                break
        pages = rng.randint(120, 720) if rng.random() > 0.15 else None
        db.execute(
            "INSERT INTO books VALUES (?, ?, ?, ?, ?, ?, ?)",
            (
                i,
                title,
                rng.randint(1, 12),
                rng.choice(GENRES),
                round(rng.uniform(5.99, 39.99), 2),
                rng.randint(1998, 2024),
                pages,
            ),
        )

    # Customers - some without email
    used_names = set()
    for i in range(1, 31):
        while True:
            name = f"{rng.choice(FIRST)} {rng.choice(LAST)}"
            if name not in used_names:
                used_names.add(name)
                break
        email = None
        if rng.random() > 0.2:
            email = name.lower().replace(" ", ".") + "@example.com"
        joined = f"{rng.randint(2021, 2024)}-{rng.randint(1, 12):02d}-{rng.randint(1, 28):02d}"
        db.execute(
            "INSERT INTO customers VALUES (?, ?, ?, ?, ?)",
            (i, name, email, rng.choice(CITIES), joined),
        )

    # Orders across 2023-2024; ~25 customers ever order (some never do)
    ordering_customers = rng.sample(range(1, 31), 25)
    statuses = ["delivered"] * 7 + ["shipped"] * 2 + ["cancelled"]
    order_id = 0
    for _ in range(120):
        order_id += 1
        year = rng.choice([2023, 2023, 2024, 2024, 2024])
        date = f"{year}-{rng.randint(1, 12):02d}-{rng.randint(1, 28):02d}"
        db.execute(
            "INSERT INTO orders VALUES (?, ?, ?, ?)",
            (order_id, rng.choice(ordering_customers), date, rng.choice(statuses)),
        )

    # Order items - unit price is the book price with occasional discount
    prices = {
        row[0]: row[1] for row in db.execute("SELECT id, price FROM books")
    }
    item_id = 0
    for oid in range(1, order_id + 1):
        for book_id in rng.sample(range(1, 41), rng.randint(1, 4)):
            item_id += 1
            unit = prices[book_id]
            if rng.random() < 0.2:
                unit = round(unit * 0.9, 2)
            db.execute(
                "INSERT INTO order_items VALUES (?, ?, ?, ?, ?)",
                (item_id, oid, book_id, rng.randint(1, 3), unit),
            )

    db.commit()
    return db


def q1(db, sql):
    """Single scalar."""
    return db.execute(sql).fetchone()[0]


def build_dataset(db):
    items = []

    def add(question, gold_sql, checks=None, gold_answer=None):
        rows = db.execute(gold_sql).fetchall()
        if checks is None:
            checks = [v for row in rows for v in row]
        if gold_answer is None:
            gold_answer = "; ".join(
                ", ".join(str(v) for v in row) for row in rows
            )
        items.append(
            {
                "question": question,
                "gold_sql": gold_sql.strip(),
                "gold_answer": str(gold_answer),
                "checks": checks,
            }
        )

    add(
        "How many books are in the database?",
        "SELECT COUNT(*) FROM books",
    )
    add(
        "How many different genres of books do we carry?",
        "SELECT COUNT(DISTINCT genre) FROM books",
    )
    author, nbooks = db.execute(
        "SELECT a.name, COUNT(*) c FROM books b JOIN authors a ON a.id = b.author_id "
        "GROUP BY a.id ORDER BY c DESC, a.name LIMIT 1"
    ).fetchone()
    add(
        "Which author has written the most books in this database, and how many?",
        "SELECT a.name, COUNT(*) c FROM books b JOIN authors a ON a.id = b.author_id "
        "GROUP BY a.id ORDER BY c DESC LIMIT 1",
        checks=[author, nbooks],
        gold_answer=f"{author} ({nbooks} books)",
    )
    total = round(
        q1(
            db,
            "SELECT SUM(quantity * unit_price) FROM order_items oi "
            "JOIN orders o ON o.id = oi.order_id WHERE o.status != 'cancelled'",
        ),
        2,
    )
    add(
        "What is the total revenue from all orders that were not cancelled? "
        "Revenue is quantity times unit price of each order item.",
        "SELECT ROUND(SUM(quantity * unit_price), 2) FROM order_items oi "
        "JOIN orders o ON o.id = oi.order_id WHERE o.status != 'cancelled'",
        checks=[total],
        gold_answer=str(total),
    )
    name, spent = db.execute(
        "SELECT c.name, ROUND(SUM(oi.quantity * oi.unit_price), 2) s "
        "FROM order_items oi JOIN orders o ON o.id = oi.order_id "
        "JOIN customers c ON c.id = o.customer_id "
        "WHERE o.order_date LIKE '2024%' AND o.status != 'cancelled' "
        "GROUP BY c.id ORDER BY s DESC LIMIT 1"
    ).fetchone()
    add(
        "Which customer spent the most money on non-cancelled orders placed "
        "in 2024, and how much did they spend?",
        "SELECT c.name, ROUND(SUM(oi.quantity * oi.unit_price), 2) s "
        "FROM order_items oi JOIN orders o ON o.id = oi.order_id "
        "JOIN customers c ON c.id = o.customer_id "
        "WHERE o.order_date LIKE '2024%' AND o.status != 'cancelled' "
        "GROUP BY c.id ORDER BY s DESC LIMIT 1",
        checks=[name, spent],
        gold_answer=f"{name} ({spent})",
    )
    genre, copies = db.execute(
        "SELECT b.genre, SUM(oi.quantity) c FROM order_items oi "
        "JOIN books b ON b.id = oi.book_id GROUP BY b.genre "
        "ORDER BY c DESC LIMIT 1"
    ).fetchone()
    add(
        "Which genre has sold the most copies overall (by total quantity "
        "across all order items)?",
        "SELECT b.genre, SUM(oi.quantity) c FROM order_items oi "
        "JOIN books b ON b.id = oi.book_id GROUP BY b.genre "
        "ORDER BY c DESC LIMIT 1",
        checks=[genre],
        gold_answer=genre,
    )
    add(
        "How many customers do not have an email address on file?",
        "SELECT COUNT(*) FROM customers WHERE email IS NULL",
    )
    avg_price = round(q1(db, "SELECT AVG(price) FROM books"), 2)
    add(
        "What is the average price of a book, rounded to two decimal places?",
        "SELECT ROUND(AVG(price), 2) FROM books",
        checks=[avg_price],
        gold_answer=str(avg_price),
    )
    month, cnt = db.execute(
        "SELECT strftime('%m', order_date) m, COUNT(*) c FROM orders "
        "WHERE order_date LIKE '2024%' GROUP BY m ORDER BY c DESC, m LIMIT 1"
    ).fetchone()
    month_names = {
        "01": "January", "02": "February", "03": "March", "04": "April",
        "05": "May", "06": "June", "07": "July", "08": "August",
        "09": "September", "10": "October", "11": "November",
        "12": "December",
    }
    add(
        "Which calendar month of 2024 had the most orders placed, and how many orders was that?",
        "SELECT strftime('%m', order_date) m, COUNT(*) c FROM orders "
        "WHERE order_date LIKE '2024%' GROUP BY m ORDER BY c DESC LIMIT 1",
        checks=[month_names[month], cnt],
        gold_answer=f"{month_names[month]} ({cnt} orders)",
    )
    add(
        "How many orders were cancelled?",
        "SELECT COUNT(*) FROM orders WHERE status = 'cancelled'",
    )
    title, price = db.execute(
        "SELECT title, price FROM books ORDER BY price ASC LIMIT 1"
    ).fetchone()
    add(
        "What is the cheapest book we sell, and what does it cost?",
        "SELECT title, price FROM books ORDER BY price ASC LIMIT 1",
        checks=[title, price],
        gold_answer=f"{title} ({price})",
    )
    add(
        "How many books were written by authors from Japan?",
        "SELECT COUNT(*) FROM books b JOIN authors a ON a.id = b.author_id "
        "WHERE a.country = 'Japan'",
    )
    ranked = db.execute(
        "SELECT b.title, SUM(oi.quantity) c FROM order_items oi "
        "JOIN books b ON b.id = oi.book_id "
        "GROUP BY b.id ORDER BY c DESC, b.title"
    ).fetchall()
    # Any title tied with the 3rd-place count is an acceptable 3rd answer.
    cutoff = ranked[2][1]
    top3_checks = [row[0] for row in ranked if row[1] > cutoff]
    tied = [row[0] for row in ranked if row[1] == cutoff]
    if len(tied) == 1:
        top3_checks.extend(tied)
    else:
        top3_checks.append({"any": tied})
    add(
        "What are the top 3 best-selling books by total copies sold?",
        "SELECT b.title, SUM(oi.quantity) c FROM order_items oi "
        "JOIN books b ON b.id = oi.book_id GROUP BY b.id "
        "ORDER BY c DESC LIMIT 3",
        checks=top3_checks,
        gold_answer=", ".join(row[0] for row in ranked[:3])
        + (f" (3rd place is a tie on {cutoff} copies: {', '.join(tied)})" if len(tied) > 1 else ""),
    )
    add(
        "How many customers have never placed an order?",
        "SELECT COUNT(*) FROM customers c LEFT JOIN orders o "
        "ON o.customer_id = c.id WHERE o.id IS NULL",
    )
    never_ordered = q1(
        db,
        "SELECT COUNT(*) FROM books b LEFT JOIN order_items oi "
        "ON oi.book_id = b.id WHERE oi.id IS NULL",
    )
    never_checks = [never_ordered]
    if never_ordered == 0:
        # Accept natural phrasings of "zero" - "none", "no books have..."
        never_checks = [{"any": [0, "none", "no books", "all books have been ordered", "every book has been ordered"]}]
    year, ybooks = db.execute(
        "SELECT published_year, COUNT(*) c FROM books GROUP BY published_year "
        "ORDER BY c DESC, published_year LIMIT 1"
    ).fetchone()
    add(
        "Which publication year appears most often among our books?",
        "SELECT published_year, COUNT(*) c FROM books GROUP BY published_year "
        "ORDER BY c DESC LIMIT 1",
        checks=[year],
        gold_answer=str(year),
    )
    rev = round(
        q1(
            db,
            "SELECT SUM(oi.quantity * oi.unit_price) FROM order_items oi "
            "JOIN books b ON b.id = oi.book_id WHERE b.genre = 'Mystery'",
        ),
        2,
    )
    add(
        "How much revenue have Mystery books generated in total (across all "
        "orders, including cancelled ones)?",
        "SELECT ROUND(SUM(oi.quantity * oi.unit_price), 2) FROM order_items oi "
        "JOIN books b ON b.id = oi.book_id WHERE b.genre = 'Mystery'",
        checks=[rev],
        gold_answer=str(rev),
    )
    city, ccount = db.execute(
        "SELECT city, COUNT(*) c FROM customers GROUP BY city "
        "ORDER BY c DESC, city LIMIT 1"
    ).fetchone()
    add(
        "Which city has the most customers?",
        "SELECT city, COUNT(*) c FROM customers GROUP BY city "
        "ORDER BY c DESC LIMIT 1",
        checks=[city],
        gold_answer=f"{city} ({ccount} customers)",
    )
    oldest, byear = db.execute(
        "SELECT name, birth_year FROM authors ORDER BY birth_year ASC LIMIT 1"
    ).fetchone()
    add(
        "Who is the oldest author in the database and what year were they born?",
        "SELECT name, birth_year FROM authors ORDER BY birth_year ASC LIMIT 1",
        checks=[oldest, byear],
        gold_answer=f"{oldest} ({byear})",
    )
    add(
        "How many books have never been ordered by anyone?",
        "SELECT COUNT(*) FROM books b LEFT JOIN order_items oi "
        "ON oi.book_id = b.id WHERE oi.id IS NULL",
        checks=never_checks,
        gold_answer=str(never_ordered),
    )
    add(
        "How many books do we not know the page count for?",
        "SELECT COUNT(*) FROM books WHERE pages IS NULL",
    )
    longest, pgs = db.execute(
        "SELECT title, pages FROM books WHERE pages IS NOT NULL "
        "ORDER BY pages DESC LIMIT 1"
    ).fetchone()
    add(
        "What is the longest book by page count?",
        "SELECT title, pages FROM books WHERE pages IS NOT NULL "
        "ORDER BY pages DESC LIMIT 1",
        checks=[longest, pgs],
        gold_answer=f"{longest} ({pgs} pages)",
    )
    add(
        "How many orders included more than 2 distinct books?",
        "SELECT COUNT(*) FROM (SELECT order_id FROM order_items "
        "GROUP BY order_id HAVING COUNT(DISTINCT book_id) > 2)",
    )
    add(
        "How many distinct customers placed at least one order in 2023?",
        "SELECT COUNT(DISTINCT customer_id) FROM orders "
        "WHERE order_date LIKE '2023%'",
    )
    aname, arev = db.execute(
        "SELECT a.name, ROUND(SUM(oi.quantity * oi.unit_price), 2) r "
        "FROM order_items oi JOIN books b ON b.id = oi.book_id "
        "JOIN authors a ON a.id = b.author_id "
        "GROUP BY a.id ORDER BY r DESC LIMIT 1"
    ).fetchone()
    add(
        "Which author's books have generated the most total revenue?",
        "SELECT a.name, ROUND(SUM(oi.quantity * oi.unit_price), 2) r "
        "FROM order_items oi JOIN books b ON b.id = oi.book_id "
        "JOIN authors a ON a.id = b.author_id "
        "GROUP BY a.id ORDER BY r DESC LIMIT 1",
        checks=[aname],
        gold_answer=f"{aname} ({arev})",
    )
    add(
        "How many order items were sold at a discount, meaning the unit "
        "price was lower than the book's list price?",
        "SELECT COUNT(*) FROM order_items oi JOIN books b ON b.id = oi.book_id "
        "WHERE oi.unit_price < b.price",
    )
    add(
        "How many authors have no books in the Science Fiction genre?",
        "SELECT COUNT(*) FROM authors a WHERE a.id NOT IN "
        "(SELECT author_id FROM books WHERE genre = 'Science Fiction')",
    )
    avg_items = round(
        q1(
            db,
            "SELECT AVG(n) FROM (SELECT SUM(quantity) n FROM order_items "
            "GROUP BY order_id)",
        ),
        2,
    )
    add(
        "On average, how many total copies (sum of quantities) does an order "
        "contain? Round to two decimal places.",
        "SELECT ROUND(AVG(n), 2) FROM (SELECT SUM(quantity) n "
        "FROM order_items GROUP BY order_id)",
        checks=[avg_items],
        gold_answer=str(avg_items),
    )
    add(
        "How many customers joined in 2022 or earlier?",
        "SELECT COUNT(*) FROM customers WHERE joined_date <= '2022-12-31'",
    )
    fr = db.execute(
        "SELECT COUNT(DISTINCT a.id) FROM authors a JOIN books b "
        "ON b.author_id = a.id WHERE a.country = 'France' "
        "AND b.published_year >= 2015"
    ).fetchone()[0]
    add(
        "How many French authors have published at least one book in 2015 "
        "or later?",
        "SELECT COUNT(DISTINCT a.id) FROM authors a JOIN books b "
        "ON b.author_id = a.id WHERE a.country = 'France' "
        "AND b.published_year >= 2015",
        checks=[fr],
        gold_answer=str(fr),
    )
    add(
        "What percentage of all orders were delivered? Round to one decimal "
        "place.",
        "SELECT ROUND(100.0 * SUM(status = 'delivered') / COUNT(*), 1) "
        "FROM orders",
    )
    return items


def main():
    db = build_db()
    items = build_dataset(db)
    shuffler = random.Random(7)
    shuffler.shuffle(items)
    for i, item in enumerate(items):
        item["split"] = "train" if i < 20 else "test"
    DATASET_PATH.write_text(json.dumps(items, indent=2, default=str))
    train = sum(1 for i in items if i["split"] == "train")
    print(f"Wrote {DB_PATH.name} and {DATASET_PATH.name}: "
          f"{len(items)} questions ({train} train, {len(items) - train} test)")
    for item in items:
        print(f"[{item['split']}] {item['question']}  -> {item['gold_answer']}")


if __name__ == "__main__":
    main()
