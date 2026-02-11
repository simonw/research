import argparse

GREETINGS = {
    "english": "Hello! Have a wonderful day!",
    "spanish": "¡Hola! ¡Que tengas un día maravilloso!",
    "japanese": "こんにちは！素敵な一日を！",
    "french": "Bonjour ! Passez une merveilleuse journée !",
    "german": "Hallo! Hab einen wundervollen Tag!",
}

parser = argparse.ArgumentParser()
parser.add_argument("--lang", default="english")
args = parser.parse_args()
lang = args.lang.lower()
print(GREETINGS.get(lang, f"Hello from the greeter skill! (no greeting for {lang})"))
