plugins {
  application
  id("iris.java-conventions")
}

dependencies { implementation(project(":iris")) }

application {
  mainClass = "dev.gecode.iris.driver.Main"
  applicationDefaultJvmArgs = listOf("-XX:TieredStopAtLevel=1", "-XX:+UseSerialGC", "-XX:-UsePerfData")
}
