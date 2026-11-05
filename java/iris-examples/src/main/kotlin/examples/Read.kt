package examples

import dev.gecode.iris.Spec
import dev.gecode.iris.kotlin.toLongArray

fun main() {
    val spec = Spec.parse("n:int row[n]:int", "example.iris")
    val data = spec.read("3 10 20 30", "input.txt")

    println(data["n"].asLong())
    println(data["row"].toLongArray().joinToString(" "))
}
