import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent
RUST_DIR = os.path.join(ROOT, "src", "rs")
PYTHON_DIR = os.path.join(ROOT, "src", "py")


def run(cmd, **kwargs):
    print(" ".join(cmd))
    subprocess.run(cmd, check=True, **kwargs)


def main():
    # Build rust workspace so the CFFI shared library is available
    run(["cargo", "build", "--tests", "--lib"], cwd=RUST_DIR)
    # Run rust unit tests and integration tests
    run(["cargo", "test"], cwd=RUST_DIR)

    env = os.environ.copy()
    python_paths = [
        str(PYTHON_DIR),
        str(Path(PYTHON_DIR) / "n_observer" / "src"),
    ]
    env["PYTHONPATH"] = os.pathsep.join(python_paths)
    # Run python unit and integration tests
    run(
        ["python", "-m", "pytest", "-v"],
        cwd=PYTHON_DIR,
        env=env,
    )


if __name__ == "__main__":
    main()
