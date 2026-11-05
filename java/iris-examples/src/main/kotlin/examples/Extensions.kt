package examples

import dev.gecode.iris.Spec
import dev.gecode.iris.kotlin.asSequence
import dev.gecode.iris.kotlin.get
import dev.gecode.iris.kotlin.leaves
import dev.gecode.iris.kotlin.toIntArray
import dev.gecode.iris.kotlin.toParams
import java.nio.file.Files
import java.nio.file.Path

fun main() {
    val dimacs = Spec.parse(Files.readAllBytes(Path.of("formats/dimacs-cnf/dimacs-cnf.iris")))
    val data = dimacs.read(Path.of("formats/dimacs-cnf/cases/three-variables/input.txt"))

    for (entry in data.asSequence()) {
        println("${entry.name} ${entry.value.type().text()} ${entry.value.depth()}")
    }
    println(data.find("model")?.size() ?: "no model")

    val clauses = data["clause"]
    println(clauses[1][0].asLong())
    println(clauses[1, 0].asLong())
    val row: IntArray = clauses[1].toIntArray()
    println(row.sum())
    println(clauses.asSequence().maxOf { clause -> clause.size() })
    println(clauses.leaves().count())

    val jobshop = Spec.parse(Files.readAllBytes(Path.of("formats/jobshop/jobshop.iris")))
    val input = Path.of("formats/jobshop/cases/or-library/input.txt")
    val params = mapOf("instance" to "handmade-or").toParams()
    println(jobshop.read(input, params)["n_jobs"].asLong())
}
