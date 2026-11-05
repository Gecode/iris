import { collectExampleFiles, collectReadmeCommands } from './example-files.mjs';

// java/iris-examples holds the guide's examples and their recorded output;
// ExamplesTest in that module runs each one from the repository root.
const javaExamples = ['Read', 'Instances', 'Access', 'Output', 'Errors'];
const kotlinExamples = ['Read', 'Extensions', 'Errors'];
const installCommands = { 'java-install': './gradlew publishToMavenLocal' };
const gradleSnippets = {
  'java-gradle': 'repositories {\n    mavenLocal()\n    mavenCentral()\n}\n\njava { toolchain { languageVersion = JavaLanguageVersion.of(27) } }\n\ndependencies {\n    implementation("dev.gecode:iris:0.1.0")\n}',
  'kotlin-gradle': 'dependencies {\n    implementation("dev.gecode:iris-kotlin:0.1.0")\n}',
  'kotlin-target': 'import org.jetbrains.kotlin.gradle.dsl.JvmTarget\nimport org.jetbrains.kotlin.gradle.dsl.jvm.JvmTargetValidationMode\nimport org.jetbrains.kotlin.gradle.tasks.KotlinJvmCompile\n\nkotlin { compilerOptions { jvmTarget = JvmTarget.JVM_26 } }\n\ntasks.withType<KotlinJvmCompile>().configureEach {\n    jvmTargetValidationMode = JvmTargetValidationMode.IGNORE\n}',
};
const mavenSnippets = {
  'java-maven': '<dependency>\n  <groupId>dev.gecode</groupId>\n  <artifactId>iris</artifactId>\n  <version>0.1.0</version>\n</dependency>',
  'kotlin-maven': '<dependency>\n  <groupId>dev.gecode</groupId>\n  <artifactId>iris-kotlin</artifactId>\n  <version>0.1.0</version>\n</dependency>',
};
export const javaPages = {
  'using/java/index.html': [
    ...Object.keys(installCommands),
    ...Object.keys(gradleSnippets),
    ...Object.keys(mavenSnippets),
    ...javaExamples.map(name => `java-${name}`),
    ...kotlinExamples.map(name => `kotlin-${name}`),
  ],
};

export async function collectJavaExamples() {
  const sources = (names, extension, language) => names.map(name => ({ file: `${name}.${extension}`, language, output: true }));
  return {
    ...(await collectReadmeCommands('java/README.md', installCommands)),
    ...(await collectReadmeCommands('java/README.md', gradleSnippets, 'kotlin')),
    ...(await collectReadmeCommands('java/README.md', mavenSnippets, 'xml')),
    ...(await collectExampleFiles({
      directory: 'java/iris-examples/src/main/java/examples',
      prefix: 'java',
      sources: sources(javaExamples, 'java', 'java'),
      support: ['package-info.java'],
    })),
    ...(await collectExampleFiles({
      directory: 'java/iris-examples/src/main/kotlin/examples',
      prefix: 'kotlin',
      sources: sources(kotlinExamples, 'kt', 'kotlin'),
    })),
  };
}
