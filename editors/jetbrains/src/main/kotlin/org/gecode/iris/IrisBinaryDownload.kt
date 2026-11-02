package org.gecode.iris

import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.execution.process.CapturingProcessHandler
import com.intellij.openapi.application.PathManager
import com.intellij.openapi.progress.ProgressIndicator
import com.intellij.openapi.util.io.NioFiles
import com.intellij.util.io.Decompressor
import com.intellij.util.io.HttpRequests
import java.io.IOException
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.StandardCopyOption
import java.security.MessageDigest

internal class IrisBinaryDownload(
    val version: String = requireNotNull(IrisBinaryDownload::class.java.getResourceAsStream("/iris-version.txt"))
        .bufferedReader().use { it.readText().trim() }
        .removeSuffix("-SNAPSHOT"),
    val os: String = System.getProperty("os.name"),
    val arch: String = System.getProperty("os.arch"),
    baseUrl: String = System.getProperty("iris.release.baseUrl", "https://github.com/Gecode/iris/releases/download"),
    cacheRoot: Path = PathManager.getSystemDir().resolve("iris"),
) {
    val target: String? = irisReleaseTarget(os, arch)
    val releaseUrl: String = "${baseUrl.trimEnd('/')}/rust-v$version"
    private val versionDirectory = cacheRoot.resolve(version)
    private val installDirectory: Path? = target?.let { versionDirectory.resolve(it) }
    private val executableName = if (target?.endsWith("windows-msvc") == true) "iris.exe" else "iris"
    val asset: String? = target?.let { "rust-$it.${if (executableName == "iris.exe") "zip" else "tar.gz"}" }

    fun cachedExecutable(): String? = installDirectory?.resolve(executableName)?.toFile()
        ?.takeIf { it.isFile && it.canExecute() }?.absolutePath

    fun install(indicator: ProgressIndicator): String {
        val destination = installDirectory ?: throw IOException("No Iris release is available for $os ($arch).")
        val archiveName = requireNotNull(asset)
        cachedExecutable()?.let { return it }
        Files.createDirectories(versionDirectory)
        val staging = Files.createTempDirectory(versionDirectory, ".download-")
        try {
            val archive = staging.resolve(archiveName)
            download("$archiveName.sha256", staging.resolve("$archiveName.sha256"), indicator)
            download(archiveName, archive, indicator)
            verifyChecksum(archive, staging.resolve("$archiveName.sha256"), indicator)
            val extracted = extract(archive, staging.resolve("extracted"), indicator)
            verifyVersion(extracted.resolve(executableName), indicator)
            indicator.checkCanceled()
            // Another project may have installed this release while we downloaded it.
            cachedExecutable()?.let { return it }
            Files.move(extracted, destination, StandardCopyOption.ATOMIC_MOVE)
            return destination.resolve(executableName).toString()
        } finally {
            NioFiles.deleteRecursively(staging)
        }
    }

    private fun download(name: String, destination: Path, indicator: ProgressIndicator) {
        indicator.checkCanceled()
        indicator.text = "Downloading $name"
        HttpRequests.request("$releaseUrl/$name").connect { request -> request.saveToFile(destination, indicator) }
    }

    private fun verifyChecksum(archive: Path, checksumFile: Path, indicator: ProgressIndicator) {
        indicator.text = "Verifying Iris SHA-256"
        val checksumLine = Regex("([0-9a-f]{64}) \\*${Regex.escape(archive.fileName.toString())}")
        val checksums = Files.readAllLines(checksumFile).filter { it.isNotBlank() }
        val expected = checksums.singleOrNull()?.let { checksumLine.matchEntire(it)?.groupValues?.get(1) }
            ?: throw IOException("Invalid SHA-256 checksum file for ${archive.fileName}.")
        val digest = MessageDigest.getInstance("SHA-256")
        Files.newInputStream(archive).use { input ->
            val buffer = ByteArray(65536)
            while (true) {
                indicator.checkCanceled()
                val count = input.read(buffer)
                if (count < 0) break
                digest.update(buffer, 0, count)
            }
        }
        val actual = digest.digest().joinToString("") { "%02x".format(it) }
        if (actual != expected) throw IOException("SHA-256 mismatch for ${archive.fileName}. The download was discarded.")
    }

    private fun extract(archive: Path, destination: Path, indicator: ProgressIndicator): Path {
        indicator.text = "Extracting Iris"
        val isZip = executableName == "iris.exe"
        val prefix = if (isZip) "" else "rust-$target/"
        val files = setOf(executableName, "LICENSE", "README.md")
        val entries = files.map { "$prefix$it" }.toSet()
        val decompressor = if (isZip) Decompressor.Zip(archive) else Decompressor.Tar(archive)
        decompressor.entryFilter { entry ->
            indicator.checkCanceled()
            entry.type == Decompressor.Entry.Type.FILE && entry.name in entries
        }.extract(destination)
        val packageDirectory = if (isZip) destination else destination.resolve("rust-$target")
        for (name in files) {
            if (!Files.isRegularFile(packageDirectory.resolve(name))) {
                throw IOException("Iris archive is missing $name.")
            }
        }
        if (!packageDirectory.resolve(executableName).toFile().setExecutable(true, true)) {
            throw IOException("Cannot make the downloaded Iris binary executable.")
        }
        return packageDirectory
    }

    private fun verifyVersion(executable: Path, indicator: ProgressIndicator) {
        indicator.text = "Checking Iris version"
        val output = CapturingProcessHandler(GeneralCommandLine(executable.toString(), "--version"))
            .runProcessWithProgressIndicator(indicator, 10_000, true)
        indicator.checkCanceled()
        val expectedVersion = Regex("iris ${Regex.escape(version)}(?: \\(languages: [^\\r\\n]+\\))?")
        if (output.isTimeout || output.exitCode != 0 || !expectedVersion.matches(output.stdout.trim())) {
            throw IOException("Downloaded iris did not report version $version; it was discarded.")
        }
    }
}

internal fun irisReleaseTarget(os: String, arch: String): String? {
    val cpu = when (arch.lowercase()) {
        "amd64", "x86_64" -> "x86_64"
        "aarch64", "arm64" -> "aarch64"
        else -> return null
    }
    return when {
        os.startsWith("Linux", ignoreCase = true) -> "$cpu-unknown-linux-gnu"
        os.startsWith("Mac", ignoreCase = true) -> "$cpu-apple-darwin"
        os.startsWith("Windows", ignoreCase = true) && cpu == "x86_64" -> "$cpu-pc-windows-msvc"
        else -> null
    }
}
