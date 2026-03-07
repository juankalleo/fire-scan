import sqlite3
import os

db = os.path.expandvars(r"%LOCALAPPDATA%\\firescan\\firescan\\data\\app.db")
print("DB path:", db)
if not os.path.exists(db):
    print("DB not found")
    raise SystemExit(1)

conn = sqlite3.connect(db)
cur = conn.cursor()
print("manga schema:")
for row in cur.execute("PRAGMA table_info('manga')"):
    print(row)
conn.close()
