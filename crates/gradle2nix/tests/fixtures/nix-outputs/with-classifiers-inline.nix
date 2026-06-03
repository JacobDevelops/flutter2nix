{
  "com.google.guava:guava:31.1-jre" = fetchMaven {
    repo = "https://repo.maven.apache.org/maven2/";
    artifact = "com/google/guava/guava/31.1-jre/guava-31.1-jre.jar";
    sha256 = "c4b87ce48bf565fce5dfc5db0ce5f13f69e58e6b0f7b960e42ab2e42e7cef83c";
  };

  "com.google.guava:guava:31.1-jre:sources" = fetchMaven {
    repo = "https://repo.maven.apache.org/maven2/";
    artifact = "com/google/guava/guava/31.1-jre/guava-31.1-jre-sources.jar";
    sha256 = "db36ac8a9a8c67e51a3dc8b6e1ab5d7e19c92c3d6e9f3c40c8f5e1b6a7d2e9c1";
  };
}
