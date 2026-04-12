plugins {
  java
}

repositories {
  mavenCentral()
}
java {
  toolchain {
    languageVersion.set(JavaLanguageVersion.of(26))
  }
}

sourceSets {
  main {
    java {
      setSrcDirs(listOf("java"))
    }
  }
}