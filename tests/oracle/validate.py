import sys

from pxr import Usd

try:
    from pxr import UsdValidation
except ImportError:
    print("this usd-core has no UsdValidation; nothing checked")
    sys.exit(0)


def main(paths):
    validators = UsdValidation.ValidationRegistry().GetOrLoadAllValidators()
    failed = 0
    for path in paths:
        stage = Usd.Stage.Open(path)
        issues = UsdValidation.ValidationContext(validators).Validate(stage)
        errors = [i for i in issues if i.GetType() == UsdValidation.ValidationErrorType.Error]
        print(f"{'ok' if not errors else 'FAIL'} {path} ({len(issues)} issues)")
        for issue in issues:
            print(f"    {issue.GetType()} {issue.GetName()}: {issue.GetMessage()}")
        failed += bool(errors)
    print(f"{len(paths) - failed}/{len(paths)} valid with {len(validators)} validators")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
