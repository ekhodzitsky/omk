import argparse


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("name")
    args = parser.parse_args()
    print(f"hello {args.name.lower()}")


if __name__ == "__main__":
    main()
