package examples

import dev.gecode.iris.IrisError
import dev.gecode.iris.IrisException
import dev.gecode.iris.Spec

fun main() {
    try {
        Spec.parse("n:int", "example.iris").read("many", "input.txt")
    } catch (e: IrisException) {
        println(e.code().code())
        println("${e.line()}:${e.column()}")
        println(
            when (val error = e.error()) {
                is IrisError.SpecError -> "the reader spec is wrong"
                is IrisError.ReadError -> "the input does not match"
                is IrisError.CheckError -> "a check failed at ${error.recordPath()}"
            }
        )
    }
}
