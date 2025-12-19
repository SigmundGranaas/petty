<xsl:stylesheet version="1.0"
                xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
                xmlns:fo="http://www.w3.org/1999/XSL/Format"
                xmlns:petty="https://petty.rs/ns/1.0">

    <!-- Page Layout -->
    <fo:simple-page-master page-width="8.5in" page-height="11in" margin="1in"/>

    <!--
        ROLE TEMPLATE: Table of Contents
    -->
    <xsl:template match="/" petty:role="table-of-contents">
        <fo:block>
            <h1 style="font-size: 24pt; font-weight: bold; text-align: center; margin-bottom: 36pt; border-bottom: 2px solid #000000; padding-bottom: 10pt;">
                Table of Contents
            </h1>

            <table width="100%">
                <columns>
                    <column width="90%"/>
                    <column width="10%"/>
                </columns>
                <xsl:for-each select="*/headings/item">
                    <row>
                        <cell>
                            <div style="margin-bottom: 4pt;">
                                <!-- Visual Grouping Logic -->
                                <!-- Add extra top margin if this is a Level 1 heading,
                                     UNLESS it is the very first item in the list -->
                                <xsl:if test="level = 1 and position() > 1">
                                    <xsl:attribute name="margin-top">16pt</xsl:attribute>
                                </xsl:if>

                                <!-- Add extra top margin if hierarchy changes from deeper to shallower (e.g. H3 -> H2) -->
                                <xsl:if test="level &lt; preceding-sibling::item[1]/level">
                                    <xsl:attribute name="margin-top">8pt</xsl:attribute>
                                </xsl:if>

                                <xsl:attribute name="margin-left">
                                    <xsl:value-of select="(level - 1) * 20"/>pt
                                </xsl:attribute>

                                <a style="text-decoration: none; color: #333333;">
                                    <xsl:attribute name="href">#<xsl:value-of select="id"/></xsl:attribute>

                                    <xsl:if test="level = 1">
                                        <xsl:attribute name="font-weight">bold</xsl:attribute>
                                        <xsl:attribute name="font-size">12pt</xsl:attribute>
                                    </xsl:if>
                                    <xsl:value-of select="text"/>
                                </a>
                            </div>
                        </cell>
                        <cell>
                            <div style="text-align: right; margin-bottom: 4pt;">
                                <!-- Align margin-top with the text cell for visual consistency -->
                                <xsl:if test="level = 1 and position() > 1">
                                    <xsl:attribute name="margin-top">16pt</xsl:attribute>
                                </xsl:if>
                                <xsl:if test="level &lt; preceding-sibling::item[1]/level">
                                    <xsl:attribute name="margin-top">8pt</xsl:attribute>
                                </xsl:if>

                                <a style="text-decoration: none; color: #666666;">
                                    <xsl:attribute name="href">#<xsl:value-of select="id"/></xsl:attribute>
                                    <xsl:if test="level = 1">
                                        <xsl:attribute name="font-weight">bold</xsl:attribute>
                                    </xsl:if>
                                    <xsl:value-of select="pageNumber"/>
                                </a>
                            </div>
                        </cell>
                    </row>
                </xsl:for-each>
            </table>
        </fo:block>
    </xsl:template>

    <!-- MAIN TEMPLATE -->
    <xsl:template match="/*">
        <fo:block>
            <h1 style="font-size: 28pt; font-weight: bold; margin-bottom: 30pt; color: #2c3e50;">
                <xsl:value-of select="documentTitle"/>
            </h1>
            <xsl:apply-templates select="sections/item"/>
        </fo:block>
    </xsl:template>

    <xsl:template match="sections/item">
        <div style="margin-bottom: 20pt;">
            <h2 style="font-size: 18pt; font-weight: bold; margin-bottom: 10pt; color: #34495e; border-bottom: 1px solid #eeeeee;">
                <xsl:attribute name="id"><xsl:value-of select="id"/></xsl:attribute>
                <xsl:value-of select="title"/>
            </h2>
            <p style="text-align: justify;">
                <xsl:value-of select="content"/>
            </p>
            <xsl:apply-templates select="subsections/item"/>
        </div>
    </xsl:template>

    <xsl:template match="subsections/item">
        <div style="margin-top: 15pt; margin-left: 10pt;">
            <h3 style="font-size: 14pt; font-weight: bold; margin-bottom: 8pt; color: #7f8c8d;">
                <xsl:attribute name="id"><xsl:value-of select="id"/></xsl:attribute>
                <xsl:value-of select="title"/>
            </h3>
            <p>
                <xsl:value-of select="content"/>
            </p>
        </div>
    </xsl:template>
</xsl:stylesheet>