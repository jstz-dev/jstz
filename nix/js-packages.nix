{
  lib,
  nix-gitignore,
  buildNpmPackage,
}: let
  # This is used to avoid rebuilding all the JS packages if only Rust code changes
  src =
    lib.sourceByRegex (nix-gitignore.gitignoreSource [] ../.) ["^.prettierignore$" "^.prettierrc$" "^package-lock.json$" "^package.json$" "^tsconfig.json$" "^packages.*$"];

  mkNpmBuild = name: "npm run --workspace=@jstz-dev/${name} build";

  jsPackage = {
    name,
    npmBuild ? mkNpmBuild name,
  }:
    buildNpmPackage {
      inherit name src npmBuild;
    };
in {
  packages = {
    js_jstz = jsPackage {name = "jstz";};
    js_types = jsPackage {
      name = "types";
      npmBuild = "";
    };
  };
}
