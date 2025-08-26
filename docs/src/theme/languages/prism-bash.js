(function (Prism) {
  // Highlight blocks like <PLACEHOLDER> or <ALIAS|ADDRESS>
  // But not <A B>
  const placeholderRegex = new RegExp("<[\\w\|]+>", "gm");

  const languageBase = {
    variable: {
      pattern: placeholderRegex,
    },
  };

  Prism.languages.bash = languageBase;
  Prism.languages.sh = languageBase;
  Prism.languages.shell = languageBase;
})(Prism);
